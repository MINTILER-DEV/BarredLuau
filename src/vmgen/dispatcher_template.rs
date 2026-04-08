pub fn emit_dispatcher() -> &'static str {
    r#"
local function readOperand(frame, operand)
    local tag = operand.tag
    if tag == 0 then
        return nil
    elseif tag == 1 then
        return frame.registers[operand.value + 1].value
    elseif tag == 2 then
        return frame.proto.constants[operand.value + 1]
    elseif tag == 3 then
        return operand.value
    elseif tag == 5 then
        return frame.upvalues[operand.value + 1].value
    elseif tag == 6 then
        return operand.value ~= 0
    end
    return nil
end

local function writeRegister(frame, registerIndex, value)
    frame.registers[registerIndex + 1].value = value
end

local function captureClosure(frame, proto, prototypes)
    local captured = {}
    local upvalueMap = {}
    for index, name in ipairs(proto.upvalues) do
        local cell = frame.namedLocals[name]
        if cell == nil then
            cell = frame.upvalueMap[name]
        end
        if cell == nil then
            cell = { value = frame.env[name] }
        end
        captured[index] = cell
        upvalueMap[name] = cell
    end

    return function(...)
        return executeProto(proto, prototypes, frame.env, captured, upvalueMap, { ... })
    end
end

function executeProto(proto, prototypes, env, upvalues, upvalueMap, args)
    local frame = {
        proto = proto,
        env = env,
        upvalues = upvalues or {},
        upvalueMap = upvalueMap or {},
        namedLocals = {},
        registers = {},
        pc = 1,
    }

    for index = 1, proto.maxRegisters do
        frame.registers[index] = { value = nil }
    end

    for index, value in ipairs(args or {}) do
        if frame.registers[index] then
            frame.registers[index].value = value
        end
    end

    for index, name in ipairs(proto.localNames) do
        if name ~= false and name ~= nil and frame.registers[index] then
            frame.namedLocals[name] = frame.registers[index]
        end
    end

    while true do
        local instruction = proto.instructions[frame.pc]
        frame.pc += 1
        if instruction == nil then
            return nil
        end

        local op = instruction.op
        if op == OPCODES.LoadNil then
            writeRegister(frame, instruction.a.value, nil)
        elseif op == OPCODES.LoadBool then
            writeRegister(frame, instruction.a.value, instruction.b.value ~= 0)
        elseif op == OPCODES.LoadNumber or op == OPCODES.LoadString then
            writeRegister(frame, instruction.a.value, proto.constants[instruction.b.value + 1])
        elseif op == OPCODES.Move then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b))
        elseif op == OPCODES.GetGlobal then
            local name = proto.constants[instruction.b.value + 1]
            writeRegister(frame, instruction.a.value, env[name])
        elseif op == OPCODES.SetGlobal then
            local name = proto.constants[instruction.a.value + 1]
            env[name] = readOperand(frame, instruction.b)
        elseif op == OPCODES.NewTable then
            writeRegister(frame, instruction.a.value, {})
        elseif op == OPCODES.GetTable then
            local tbl = readOperand(frame, instruction.b)
            local key = readOperand(frame, instruction.c)
            writeRegister(frame, instruction.a.value, tbl[key])
        elseif op == OPCODES.SetTable then
            local tbl = readOperand(frame, instruction.a)
            local key = readOperand(frame, instruction.b)
            tbl[key] = readOperand(frame, instruction.c)
        elseif op == OPCODES.Call then
            local base = instruction.a.value
            local callee = frame.registers[base + 1].value
            local argsBuffer = {}
            for argIndex = 1, instruction.b.value do
                argsBuffer[argIndex] = frame.registers[base + argIndex + 1].value
            end
            writeRegister(frame, instruction.c.value, callee(table.unpack(argsBuffer)))
        elseif op == OPCODES.Return then
            local count = instruction.b.value
            if count == 0 then
                return nil
            elseif count == 1 then
                return frame.registers[instruction.a.value + 1].value
            else
                local results = {}
                for resultIndex = 1, count do
                    results[resultIndex] = frame.registers[instruction.a.value + resultIndex].value
                end
                return table.unpack(results)
            end
        elseif op == OPCODES.Jump then
            frame.pc += instruction.b.value
        elseif op == OPCODES.JumpIf then
            if readOperand(frame, instruction.a) then
                frame.pc += instruction.b.value
            end
        elseif op == OPCODES.JumpIfNot then
            if not readOperand(frame, instruction.a) then
                frame.pc += instruction.b.value
            end
        elseif op == OPCODES.Closure then
            local child = prototypes[instruction.b.value + 1]
            writeRegister(frame, instruction.a.value, captureClosure(frame, child, prototypes))
        elseif op == OPCODES.GetUpvalue then
            writeRegister(frame, instruction.a.value, frame.upvalues[instruction.b.value + 1].value)
        elseif op == OPCODES.SetUpvalue then
            frame.upvalues[instruction.a.value + 1].value = readOperand(frame, instruction.b)
        elseif op == OPCODES.Concat then
            writeRegister(frame, instruction.a.value, tostring(readOperand(frame, instruction.b)) .. tostring(readOperand(frame, instruction.c)))
        elseif op == OPCODES.Add then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) + readOperand(frame, instruction.c))
        elseif op == OPCODES.Sub then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) - readOperand(frame, instruction.c))
        elseif op == OPCODES.Mul then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) * readOperand(frame, instruction.c))
        elseif op == OPCODES.Div then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) / readOperand(frame, instruction.c))
        elseif op == OPCODES.Mod then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) % readOperand(frame, instruction.c))
        elseif op == OPCODES.Pow then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) ^ readOperand(frame, instruction.c))
        elseif op == OPCODES.Eq then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) == readOperand(frame, instruction.c))
        elseif op == OPCODES.Lt then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) < readOperand(frame, instruction.c))
        elseif op == OPCODES.Le then
            writeRegister(frame, instruction.a.value, readOperand(frame, instruction.b) <= readOperand(frame, instruction.c))
        elseif op == OPCODES.Len then
            writeRegister(frame, instruction.a.value, #readOperand(frame, instruction.b))
        elseif op == OPCODES.Not then
            writeRegister(frame, instruction.a.value, not readOperand(frame, instruction.b))
        else
            error("barredluau runtime fault")
        end
    end
end
"#
}
