use omnilua::{Lua, Result, Table, Value};
use serde_json::{Map, Number, Value as Json};

use crate::err;

const MAX_DEPTH: u32 = 64;

pub fn encode(v: Value, pretty: bool) -> Result<String> {
    let j = to_json(v, 0)?;
    let s = if pretty { serde_json::to_string_pretty(&j) } else { serde_json::to_string(&j) };
    s.map_err(err)
}

pub fn decode(lua: &Lua, s: &str) -> Result<Value> {
    let j: Json = serde_json::from_str(s).map_err(err)?;
    from_json(lua, j)
}

fn to_json(v: Value, depth: u32) -> Result<Json> {
    if depth > MAX_DEPTH {
        return Err(err("table is nested too deep (or contains a cycle)"));
    }
    Ok(match v {
        Value::Nil => Json::Null,
        Value::Boolean(b) => Json::Bool(b),
        Value::Integer(i) => Json::from(i),
        // Integral floats go out as integers, otherwise serde on the server rejects `1.0` for u32 fields.
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Json::from(n as i64),
        Value::Number(n) => Number::from_f64(n).map(Json::Number).unwrap_or(Json::Null),
        Value::String(s) => Json::String(String::from_utf8_lossy(&s.as_bytes()?).into_owned()),
        Value::Table(t) => table_to_json(t, depth)?,
        other => return Err(err(format!("cannot convert {} to JSON", type_name(&other)))),
    })
}

/// A table with keys exactly 1..n becomes an array (an empty table becomes `[]`, as in GMod).
fn table_to_json(t: Table, depth: u32) -> Result<Json> {
    let pairs = t.raw_pairs()?;
    let n = pairs.len();
    let is_array = pairs.iter().all(|(k, _)| matches!(k, Value::Integer(i) if *i >= 1 && (*i as usize) <= n));
    if is_array {
        let mut items = vec![Json::Null; n];
        for (k, v) in pairs {
            if let Value::Integer(i) = k {
                items[i as usize - 1] = to_json(v, depth + 1)?;
            }
        }
        return Ok(Json::Array(items));
    }
    let mut map = Map::new();
    for (k, v) in pairs {
        let key = match k {
            Value::String(s) => String::from_utf8_lossy(&s.as_bytes()?).into_owned(),
            Value::Integer(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            other => return Err(err(format!("cannot use {} as a JSON key", type_name(&other)))),
        };
        map.insert(key, to_json(v, depth + 1)?);
    }
    Ok(Json::Object(map))
}

fn from_json(lua: &Lua, j: Json) -> Result<Value> {
    Ok(match j {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::Boolean(b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Number(n.as_f64().unwrap_or(0.0)),
        },
        Json::String(s) => Value::String(lua.create_string(s)?),
        Json::Array(items) => {
            let t = lua.create_table()?;
            for (i, item) in items.into_iter().enumerate() {
                t.raw_set(i as i64 + 1, from_json(lua, item)?)?;
            }
            Value::Table(t)
        }
        Json::Object(map) => {
            let t = lua.create_table()?;
            for (k, v) in map {
                t.raw_set(k, from_json(lua, v)?)?;
            }
            Value::Table(t)
        }
    })
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Function(_) => "function",
        Value::UserData(_) | Value::LightUserData(_) => "userdata",
        Value::Thread(_) => "thread",
        _ => "value",
    }
}
