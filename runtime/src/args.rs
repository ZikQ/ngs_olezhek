use omnilua::{FromLua, FromLuaMulti, IntoLua, IntoLuaMulti, Lua, Result, Value};

/// omnilua implements multi-value conversion only for tuples up to 3 elements;
/// wrap a longer tuple in `Args` to pass or receive up to 10 values.
pub struct Args<T>(pub T);

macro_rules! args_tuple {
    ($($name:ident),+) => {
        impl<$($name: FromLua),+> FromLuaMulti for Args<($($name,)+)> {
            const NRESULTS: i32 = [$(stringify!($name)),+].len() as i32;

            fn from_lua_multi(values: Vec<Value>, lua: &Lua) -> Result<Self> {
                let mut it = values.into_iter();
                Ok(Args(($($name::from_lua(it.next().unwrap_or(Value::Nil), lua)?,)+)))
            }
        }

        impl<$($name: IntoLua),+> IntoLuaMulti for Args<($($name,)+)> {
            #[allow(non_snake_case)]
            fn into_lua_multi(self, lua: &Lua) -> Result<Vec<Value>> {
                let ($($name,)+) = self.0;
                Ok(vec![$($name.into_lua(lua)?),+])
            }
        }
    };
}

args_tuple!(A, B, C, D);
args_tuple!(A, B, C, D, E);
args_tuple!(A, B, C, D, E, F);
args_tuple!(A, B, C, D, E, F, G);
args_tuple!(A, B, C, D, E, F, G, H);
args_tuple!(A, B, C, D, E, F, G, H, I);
args_tuple!(A, B, C, D, E, F, G, H, I, J);
