local seed = 3

local function makeAdder(x)
    return function(y)
        local payload = { result = x + y + seed, label = "ok" }
        if payload.result > 10 then
            return payload.result
        else
            return 0
        end
    end
end

local add = makeAdder(7)
return add(9)
