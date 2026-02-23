use mlua::{Lua, Result, Function};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::scripting::bindings::register_bindings;

pub struct LuaEngine {
    lua: Arc<Mutex<Lua>>,
}

impl LuaEngine {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        // Register core bindings
        register_bindings(&lua)?;
        
        Ok(Self {
            lua: Arc::new(Mutex::new(lua)),
        })
    }

    pub async fn execute(&self, script: &str) -> Result<String> {
        let lua = self.lua.lock().await;
        
        // Execute script and return result as string
        let result: String = lua.load(script).eval_async().await?;
        Ok(result)
    }

    pub async fn call_function(&self, func_name: &str, args: impl mlua::IntoLuaMulti) -> Result<String> {
        let lua = self.lua.lock().await;
        let func: Function = lua.globals().get(func_name)?;
        let result: String = func.call_async(args).await?;
        Ok(result)
    }
}
