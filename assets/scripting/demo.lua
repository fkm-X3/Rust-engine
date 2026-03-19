local t = 0.0
local spawn_timer = 0.0
local SPAWN_INTERVAL = 2.0

function on_start()
    engine.log("Demo script started!")
end

function on_update(dt)
    t = t + dt
    spawn_timer = spawn_timer + dt

    -- Log FPS every second
    if math.floor(t) > math.floor(t - dt) then
        engine.log(string.format("Time: %.1fs  |  Frame: %d", engine.time, engine.frame))
    end
end