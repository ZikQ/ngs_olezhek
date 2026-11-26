//! Untrusted Lua (plugins, user scripts) runs in a VM of its own: no table, function or upvalue
//! is shared with the app's state, so nothing in it can reach N, R or the app's globals, with
//! or without the debug library. Only plain data crosses the boundary, copied. Each top-level
//! entry gets a fresh instruction budget; memory is capped for the VM's whole life.
//!
//! Depends on omnilua alone, so tests/prelude.rs compiles this file natively.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use omnilua::{Function, Lua, LuaError, LuaString, Result, Sandbox, SandboxConfig, Table, TripReason, Value, Variadic};

const MAX_DEPTH: usize = 32;
const MAX_ITEMS: usize = 100_000;
const MAX_NESTING: u32 = 8;
const EXTRA_REMOVED: &[&[u8]] = &[b"string.dump", b"collectgarbage"];

struct Vm {
    lua: Lua,
    sandbox: Sandbox,
    instructions: u64,
    depth: Cell<u32>,
}

thread_local! {
    static VMS: RefCell<HashMap<u32, Rc<Vm>>> = RefCell::default();
    static NEXT: Cell<u32> = const { Cell::new(0) };
}

fn fail(msg: impl std::fmt::Display) -> omnilua::Error {
    LuaError::runtime(format_args!("{msg}")).into()
}

fn vm(id: u32) -> Result<Rc<Vm>> {
    VMS.with(|m| m.borrow().get(&id).cloned()).ok_or_else(|| fail("sandbox is closed"))
}

/// Copies plain data into `to`. Metatables are dropped and read raw, so no code of the source
/// side runs while copying; functions, threads and userdata are refused.
fn copy(v: Value, to: &Lua, items: &mut usize, depth: usize) -> Result<Value> {
    Ok(match v {
        Value::Nil => Value::Nil,
        Value::Boolean(b) => Value::Boolean(b),
        Value::Integer(i) => Value::Integer(i),
        Value::Number(n) => Value::Number(n),
        Value::String(s) => Value::String(to.create_string(s.as_bytes()?)?),
        Value::Table(t) => {
            if depth >= MAX_DEPTH {
                return Err(fail("table nested too deeply (or cyclic) to cross the sandbox boundary"));
            }
            let out = to.create_table()?;
            for (k, v) in t.raw_pairs()? {
                *items = items.checked_sub(1).ok_or_else(|| fail("table too large to cross the sandbox boundary"))?;
                let k = copy(k, to, items, depth + 1)?;
                if !matches!(k, Value::Nil) {
                    out.raw_set(k, copy(v, to, items, depth + 1)?)?;
                }
            }
            Value::Table(out)
        }
        other => return Err(fail(format!("a {} cannot cross the sandbox boundary", type_name(&other)))),
    })
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Function(_) => "function",
        Value::Thread(_) => "coroutine",
        _ => "userdata",
    }
}

fn copy_all(values: Vec<Value>, to: &Lua) -> Result<Vec<Value>> {
    let mut items = MAX_ITEMS;
    values.into_iter().map(|v| copy(v, to, &mut items, 0)).collect()
}

/// Runs `f` with the VM's budget refilled at the outermost entry, and turns a tripped limit
/// into a readable message.
fn enter<T>(vm: &Vm, f: impl FnOnce() -> Result<T>) -> std::result::Result<T, String> {
    let depth = vm.depth.get();
    if depth >= MAX_NESTING {
        return Err("sandbox calls nested too deeply".into());
    }
    if depth == 0 {
        vm.sandbox.reset();
    }
    vm.depth.set(depth + 1);
    let r = f();
    vm.depth.set(depth);
    r.map_err(|e| match vm.sandbox.tripped() {
        Some(TripReason::Instructions) => format!("instruction limit exceeded ({} per call)", vm.instructions),
        Some(TripReason::Memory) => "memory limit exceeded".into(),
        None => e.to_string(),
    })
}

fn create(host_lua: &Lua, instructions: u64, memory: usize, host: Function) -> Result<u32> {
    let mut remove: Vec<Vec<u8>> = SandboxConfig::strict().remove_globals;
    remove.extend(EXTRA_REMOVED.iter().map(|s| s.to_vec()));
    let (lua, sandbox) = Lua::sandboxed(SandboxConfig {
        instruction_limit: Some(instructions.max(1)),
        memory_limit_bytes: Some(memory.max(64 * 1024)),
        check_interval: 1000,
        remove_globals: remove,
    })?;
    let host_lua = host_lua.clone();
    let bridge = lua.create_function(move |box_lua: &Lua, args: Variadic<Value>| {
        let args = copy_all(args.into_vec(), &host_lua)?;
        let res: Variadic<Value> = host.call(Variadic::from(args))?;
        Ok(Variadic::from(copy_all(res.into_vec(), box_lua)?))
    })?;
    lua.globals().raw_set("__host", bridge)?;
    let id = NEXT.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    VMS.with(|m| m.borrow_mut().insert(id, Rc::new(Vm { lua, sandbox, instructions, depth: Cell::new(0) })));
    Ok(id)
}

fn run(id: u32, source: LuaString, name: String) -> Result<(bool, Option<String>)> {
    let vm = vm(id)?;
    let bytes = source.as_bytes()?;
    if bytes.first() == Some(&0x1b) {
        return Ok((false, Some("binary chunks are not allowed in the sandbox".into())));
    }
    match enter(&vm, || vm.lua.load(bytes).set_name(format!("={name}")).exec()) {
        Ok(()) => Ok((true, None)),
        Err(e) => Ok((false, Some(e))),
    }
}

/// Calls the sandbox's global `name` with copies of `args`; returns true plus copied results,
/// or false plus the error message.
fn call(host_lua: &Lua, id: u32, name: String, args: Vec<Value>) -> Result<Variadic<Value>> {
    let vm = vm(id)?;
    let args = copy_all(args, &vm.lua)?;
    let res = enter(&vm, || {
        let f: Value = vm.lua.globals().raw_get(name.as_str())?;
        let Value::Function(f) = f else { return Err(fail(format!("sandbox has no function '{name}'"))) };
        let out: Variadic<Value> = f.call(Variadic::from(args))?;
        Ok(out.into_vec())
    });
    let mut out = Vec::new();
    match res {
        Ok(values) => match copy_all(values, host_lua) {
            Ok(values) => {
                out.push(Value::Boolean(true));
                out.extend(values);
            }
            Err(e) => {
                out.push(Value::Boolean(false));
                out.push(Value::String(host_lua.create_string(e.to_string())?));
            }
        },
        Err(e) => {
            out.push(Value::Boolean(false));
            out.push(Value::String(host_lua.create_string(e)?));
        }
    }
    Ok(Variadic::from(out))
}

pub fn register(lua: &Lua, n: &Table) -> Result<()> {
    n.raw_set(
        "sandbox_new",
        lua.create_function(|lua, (instructions, memory, host): (u64, u64, Function)| {
            create(lua, instructions, usize::try_from(memory).unwrap_or(usize::MAX), host)
        })?,
    )?;
    n.raw_set("sandbox_run", lua.create_function(|_, (id, source, name): (u32, LuaString, String)| run(id, source, name))?)?;
    n.raw_set(
        "sandbox_call",
        lua.create_function(|lua, args: Variadic<Value>| {
            let mut args = args.into_vec().into_iter();
            let id = match args.next() {
                Some(Value::Integer(i)) => u32::try_from(i).map_err(|_| fail("bad sandbox id"))?,
                Some(Value::Number(f)) if f.fract() == 0.0 && f >= 0.0 => f as u32,
                _ => return Err(fail("sandbox_call: expected an id")),
            };
            let name = match args.next() {
                Some(Value::String(s)) => s.to_str()?,
                _ => return Err(fail("sandbox_call: expected a function name")),
            };
            call(lua, id, name, args.collect())
        })?,
    )?;
    n.raw_set(
        "sandbox_close",
        lua.create_function(|_, id: u32| {
            VMS.with(|m| m.borrow_mut().remove(&id));
            Ok(())
        })?,
    )?;
    Ok(())
}
