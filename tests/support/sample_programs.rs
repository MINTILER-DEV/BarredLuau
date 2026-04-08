#![allow(dead_code)]

pub fn arithmetic_and_tables() -> &'static str {
    r#"
local x = 4
local y = 8
local sum = x + y
local box = { answer = sum, "hi" }
return box.answer
"#
}

pub fn closure_capture() -> &'static str {
    r#"
local seed = 3
local function makeAdder(x)
    return function(y)
        return x + y + seed
    end
end
local add = makeAdder(7)
return add(9)
"#
}

pub fn conditionals_and_loop() -> &'static str {
    r#"
local total = 0
local i = 0
while i < 3 do
    total = total + i
    i = i + 1
end
if total == 3 then
    return "ok"
else
    return "bad"
end
"#
}
