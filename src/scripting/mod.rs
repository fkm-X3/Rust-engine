use std::path::{Path, PathBuf};

use anyhow::Result;
use log::{error, info};
use mlua::prelude::*;


// ─── Script ───────────────────────────────────────────────────────────────────

/// A loaded Lua script with its own state.
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    lua: Lua,
}

impl Script {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let lua = Lua::new();
        Self::setup_sandbox(&lua)?;

        let source = std::fs::read_to_string(&path)?;
        lua.load(&source).exec().map_err(|e| anyhow::anyhow!("Failed to load Lua script: {}", e))?;

        Ok(Self { name, path, lua })
    }

    /// Load from a string (useful for unit tests).
    pub fn from_source(name: &str, source: &str) -> Result<Self> {
        let lua = Lua::new();
        Self::setup_sandbox(&lua)?;
        lua.load(source).exec().map_err(|e| anyhow::anyhow!("Failed to load Lua source: {}", e))?;
        Ok(Self {
            name: name.to_string(),
            path: PathBuf::new(),
            lua,
        })
    }

    fn setup_sandbox(lua: &Lua) -> Result<()> {
        let globals = lua.globals();

        // Engine API table
        let engine = lua.create_table().map_err(|e| anyhow::anyhow!("Failed to create table: {}", e))?;

        engine.set("log", lua.create_function(|_, msg: String| {
            info!("[Lua] {}", msg);
            Ok(())
        }).map_err(|e| anyhow::anyhow!("Failed to create function: {}", e))?)
            .map_err(|e| anyhow::anyhow!("Failed to set log: {}", e))?;

        engine.set("warn", lua.create_function(|_, msg: String| {
            log::warn!("[Lua] {}", msg);
            Ok(())
        }).map_err(|e| anyhow::anyhow!("Failed to create function: {}", e))?)
            .map_err(|e| anyhow::anyhow!("Failed to set warn: {}", e))?;

        engine.set("time", 0.0_f64).map_err(|e| anyhow::anyhow!("Failed to set time: {}", e))?;
        engine.set("delta", 0.0_f64).map_err(|e| anyhow::anyhow!("Failed to set delta: {}", e))?;
        engine.set("frame", 0_u64).map_err(|e| anyhow::anyhow!("Failed to set frame: {}", e))?;

        globals.set("engine", engine).map_err(|e| anyhow::anyhow!("Failed to set engine global: {}", e))?;

        // Math helpers
        let math = globals.get::<LuaTable>("math").map_err(|e| anyhow::anyhow!("Failed to get math table: {}", e))?;
        math.set("vec3", lua.create_function(|_, (_x, _y, _z): (f64, f64, f64)| {
            let _t = lua_globals_table()?; // inner fn trick
            // Return a table with x/y/z
            Err::<LuaValue, _>(LuaError::RuntimeError("use glam from Rust side".into()))
        }).map_err(|e| anyhow::anyhow!("Failed to create vec3 function: {}", e))?)
            .map_err(|e| anyhow::anyhow!("Failed to set vec3: {}", e))?;
        // (We leave glam interop as a Rust-side concern; scripts get plain numbers.)

        Ok(())
    }

    /// Call the script's `on_update(dt)` function if it exists.
    pub fn on_update(&self, dt: f32, time: f32, frame: u64) -> Result<()> {
        // Update engine table values
        let globals = self.lua.globals();
        if let Ok(engine) = globals.get::<LuaTable>("engine") {
            let _ = engine.set("time",  time as f64);
            let _ = engine.set("delta", dt as f64);
            let _ = engine.set("frame", frame);
        }

        let func: LuaResult<LuaFunction> = globals.get("on_update");
        match func {
            Ok(f) => {
                f.call::<()>(dt as f64)
                    .map_err(|e| anyhow::anyhow!("Script '{}' on_update error: {}", self.name, e))
            }
            Err(_) => Ok(()), // function doesn't exist, that's fine
        }
    }

    /// Call the script's `on_start()` function if it exists.
    pub fn on_start(&self) -> Result<()> {
        let func: LuaResult<LuaFunction> = self.lua.globals().get("on_start");
        match func {
            Ok(f) => f
                .call::<()>(())
                .map_err(|e| anyhow::anyhow!("Script '{}' on_start error: {}", self.name, e)),
            Err(_) => Ok(()),
        }
    }

    /// Evaluate an arbitrary Lua expression (REPL / console use).
    pub fn eval(&self, code: &str) -> Result<String> {
        let val: LuaValue = self.lua.load(code).eval()
            .map_err(|e| anyhow::anyhow!("Failed to eval: {}", e))?;
        Ok(format!("{:?}", val))
    }
}

// ─── ScriptEngine ─────────────────────────────────────────────────────────────

/// Manages all loaded scripts and drives their lifecycle.
pub struct ScriptEngine {
    scripts: Vec<Script>,
    scripts_dir: PathBuf,
}

impl ScriptEngine {
    pub fn new() -> Self {
        Self {
            scripts: Vec::new(),
            scripts_dir: PathBuf::from("assets/scripts"),
        }
    }

    pub fn set_scripts_dir<P: Into<PathBuf>>(&mut self, dir: P) {
        self.scripts_dir = dir.into();
    }

    /// Load all `.lua` files in the scripts directory.
    pub fn load_all(&mut self) -> Result<()> {
        if !self.scripts_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.scripts_dir)? {
            let entry = entry?;
            let path  = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("lua") {
                match Script::load(&path) {
                    Ok(script) => {
                        info!("Loaded script: {}", script.name);
                        self.scripts.push(script);
                    }
                    Err(e) => error!("Failed to load script {:?}: {}", path, e),
                }
            }
        }
        Ok(())
    }

    /// Add a script loaded from source directly.
    pub fn add_script(&mut self, script: Script) {
        self.scripts.push(script);
    }

    /// Call `on_start()` on all scripts.
    pub fn start(&self) {
        for script in &self.scripts {
            if let Err(e) = script.on_start() {
                error!("{}", e);
            }
        }
    }

    /// Call `on_update(dt)` on all scripts.
    pub fn update(&self, dt: f32, time: f32, frame: u64) {
        for script in &self.scripts {
            if let Err(e) = script.on_update(dt, time, frame) {
                error!("{}", e);
            }
        }
    }

    /// Reload all scripts from disk.
    pub fn reload_all(&mut self) {
        let _dir = self.scripts_dir.clone();
        self.scripts.clear();
        if let Err(e) = self.load_all() {
            error!("Script reload failed: {}", e);
        }
    }

    pub fn script_count(&self) -> usize { self.scripts.len() }
}

impl Default for ScriptEngine {
    fn default() -> Self { Self::new() }
}

// Helper to dodge borrow issues inside closures
fn lua_globals_table() -> LuaResult<()> { Ok(()) }