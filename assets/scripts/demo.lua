-- VKEngine Demo Script
-- This script demonstrates the Lua scripting capabilities of the engine.
-- Scripts have access to the 'engine' global table with timing and logging functions.

-- Called once when the script loads
function on_start()
    engine.log("Demo script started!")
    engine.log("Available engine API:")
    engine.log("  - engine.time: Current elapsed time (seconds)")
    engine.log("  - engine.delta: Frame delta time (seconds)")
    engine.log("  - engine.frame: Current frame number")
    engine.log("  - engine.log(msg): Log info message")
    engine.log("  - engine.warn(msg): Log warning message")
end

-- Called every frame with delta time
function on_update(dt)
    -- Log a message every 5 seconds
    local current_second = math.floor(engine.time)
    
    if current_second % 5 == 0 and engine.frame % 60 < 2 then
        engine.log(string.format("🎮 Demo running for %.1f seconds at frame %d (FPS: ~%.0f)", 
            engine.time, engine.frame, 1.0 / dt))
    end
    
    -- Example of periodic warnings (every 10 seconds)
    if current_second % 10 == 0 and engine.frame % 60 < 2 then
        engine.warn("This is an example warning from Lua!")
    end
end
