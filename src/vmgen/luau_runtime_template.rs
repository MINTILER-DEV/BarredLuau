pub fn emit_runtime_support() -> &'static str {
    r#"
local bit32 = bit32

local function fnv32(bytes)
    local hash = 2166136261
    for index = 1, #bytes do
        hash = bit32.bxor(hash, bytes[index])
        hash = (hash * 16777619) % 4294967296
    end
    return hash
end

local function newPrng(seed)
    return {
        state = seed % 4294967296,
        nextU32 = function(self)
            self.state = (self.state * 1664525 + 1013904223) % 4294967296
            return self.state
        end,
        nextByte = function(self)
            return bit32.rshift(self:nextU32(), 24)
        end,
        bounded = function(self, bound)
            if bound <= 0 then
                return 0
            end
            return self:nextU32() % bound
        end,
    }
end

local function buildPermutation(length, seed)
    local permutation = {}
    for index = 1, length do
        permutation[index] = index
    end
    local prng = newPrng(seed)
    for index = length, 2, -1 do
        local target = prng:bounded(index) + 1
        local tmp = permutation[index]
        permutation[index] = permutation[target]
        permutation[target] = tmp
    end
    return permutation
end

local function invertPermutation(bytes, permutation)
    local restored = table.create(#bytes, 0)
    for outputIndex = 1, #permutation do
        restored[permutation[outputIndex]] = bytes[outputIndex]
    end
    return restored
end

local function streamTransform(bytes, seed, encode)
    local prng = newPrng(seed)
    for index = 1, #bytes do
        local add = prng:nextByte()
        local mask = prng:nextByte()
        if encode then
            bytes[index] = bit32.bxor((bytes[index] + add) % 256, mask)
        else
            bytes[index] = (bit32.bxor(bytes[index], mask) - add) % 256
        end
    end
end

local function buildSubstitution(seed)
    local tableValues = table.create(256, 0)
    for index = 1, 256 do
        tableValues[index] = index - 1
    end
    local prng = newPrng(seed)
    for index = 256, 2, -1 do
        local target = prng:bounded(index) + 1
        local tmp = tableValues[index]
        tableValues[index] = tableValues[target]
        tableValues[target] = tmp
    end
    return tableValues
end

local function invertSubstitution(substitution)
    local inverse = table.create(256, 0)
    for index = 1, 256 do
        inverse[substitution[index] + 1] = index - 1
    end
    return inverse
end

local function substitute(bytes, substitution)
    for index = 1, #bytes do
        bytes[index] = substitution[bytes[index] + 1]
    end
end

local function decodeText(text, alphabet)
    local reverse = {}
    for index = 1, #alphabet do
        reverse[string.sub(alphabet, index, index)] = index - 1
    end
    local digits = {}
    for index = 1, #text do
        local ch = string.sub(text, index, index)
        if ch ~= ":" then
            local digit = reverse[ch]
            if digit == nil then
                error("barredluau integrity check failed")
            end
            digits[#digits + 1] = digit
        end
    end
    if #digits % 2 ~= 0 then
        error("barredluau integrity check failed")
    end
    local radix = #alphabet
    local bytes = {}
    for index = 1, #digits, 2 do
        local value = digits[index] * radix + digits[index + 1]
        if value > 255 then
            error("barredluau integrity check failed")
        end
        bytes[#bytes + 1] = value
    end
    return bytes
end

local function deinterleave(bytes)
    local leftSize = math.ceil(#bytes / 2)
    local out = {}
    for index = 1, leftSize do
        out[#out + 1] = bytes[index]
        local rhs = leftSize + index
        if bytes[rhs] ~= nil then
            out[#out + 1] = bytes[rhs]
        end
    end
    return out
end

local function bytesToString(bytes)
    local parts = {}
    for offset = 1, #bytes, 4096 do
        local chunk = {}
        local upper = math.min(offset + 4095, #bytes)
        for index = offset, upper do
            chunk[#chunk + 1] = string.char(bytes[index])
        end
        parts[#parts + 1] = table.concat(chunk)
    end
    return table.concat(parts)
end

local function decodePayload(encodedBlob, runtimeKey, runtimeCfg)
    local payload = decodeText(encodedBlob, runtimeCfg.alphabet)
    for round = runtimeCfg.rounds, 1, -1 do
        local roundSeed = bit32.bxor((runtimeKey.seed + bit32.lrotate(runtimeKey.nonce, 5) + ((round - 1) * 977)) % 4294967296, 0x9E3779B9)
        local inverse = invertSubstitution(buildSubstitution(bit32.bxor(roundSeed, 0x55AA10F1)))
        substitute(payload, inverse)
        streamTransform(payload, bit32.bxor(roundSeed, 0xC0DE7705), false)
        payload = invertPermutation(payload, buildPermutation(#payload, bit32.bxor(roundSeed, 0xA17C91E3)))
    end
    if runtimeCfg.interleave then
        payload = deinterleave(payload)
    end

    local size = payload[1] + payload[2] * 256 + payload[3] * 65536 + payload[4] * 16777216
    local start = 5
    local checksum = nil
    if runtimeCfg.includeChecksum then
        checksum = payload[5] + payload[6] * 256 + payload[7] * 65536 + payload[8] * 16777216
        start = 9
    end

    local data = {}
    for index = 0, size - 1 do
        data[index + 1] = payload[start + index]
    end
    if runtimeCfg.includeChecksum and checksum ~= fnv32(data) then
        error("barredluau integrity check failed")
    end
    return data
end

local function deserializeProgram(bytes)
    local blob = bytesToString(bytes)
    local cursor = 1

    local function readByte()
        local value = string.byte(blob, cursor, cursor)
        cursor += 1
        return value
    end

    local function readU16()
        local a, b = string.byte(blob, cursor, cursor + 1)
        cursor += 2
        return a + b * 256
    end

    local function readU32()
        local a, b, c, d = string.byte(blob, cursor, cursor + 3)
        cursor += 4
        return a + b * 256 + c * 65536 + d * 16777216
    end

    local function readVarU32()
        local shift = 0
        local value = 0
        while true do
            local byte = readByte()
            value += bit32.lshift(bit32.band(byte, 0x7F), shift)
            if bit32.band(byte, 0x80) == 0 then
                return value
            end
            shift += 7
        end
    end

    local function readString()
        local size = readVarU32()
        local value = string.sub(blob, cursor, cursor + size - 1)
        cursor += size
        return value
    end

    local function readOperand()
        local tag = readByte()
        if tag == 0 then
            return { tag = 0, value = 0 }
        elseif tag == 1 or tag == 2 or tag == 4 or tag == 5 then
            return { tag = tag, value = readVarU32() }
        elseif tag == 3 then
            local value, nextCursor = string.unpack("<i4", blob, cursor)
            cursor = nextCursor
            return { tag = tag, value = value }
        elseif tag == 6 then
            return { tag = tag, value = readByte() }
        end
        error("barredluau integrity check failed")
    end

    local magic = string.sub(blob, cursor, cursor + 3)
    cursor += 4
    local version = readU16()
    local featureFlags = readU32()
    local entry = readVarU32()
    local protoCount = readVarU32()
    local prototypes = {}
    for protoIndex = 1, protoCount do
        local hasName = readByte() == 1
        local name = hasName and readString() or nil
        local paramCount = readVarU32()
        local params = {}
        for index = 1, paramCount do
            params[index] = readString()
        end
        local maxRegisters = readU16()
        local returnArity = readByte()
        local upvalueCount = readVarU32()
        local upvalues = {}
        for index = 1, upvalueCount do
            upvalues[index] = readString()
        end
        local childCount = readVarU32()
        local children = {}
        for index = 1, childCount do
            children[index] = readVarU32()
        end
        local localNameCount = readVarU32()
        local localNames = {}
        for index = 1, localNameCount do
            localNames[index] = readByte() == 1 and readString() or false
        end
        local constantCount = readVarU32()
        local constants = {}
        for index = 1, constantCount do
            local tag = readByte()
            if tag == 0 then
                constants[index] = nil
            elseif tag == 1 then
                constants[index] = readByte() == 1
            elseif tag == 2 then
                local value, nextCursor = string.unpack("<d", blob, cursor)
                cursor = nextCursor
                constants[index] = value
            elseif tag == 3 then
                constants[index] = readString()
            else
                error("barredluau integrity check failed")
            end
        end
        local instructionCount = readVarU32()
        local instructions = {}
        for index = 1, instructionCount do
            instructions[index] = {
                op = readU16(),
                a = readOperand(),
                b = readOperand(),
                c = readOperand(),
            }
        end
        prototypes[protoIndex] = {
            name = name,
            params = params,
            maxRegisters = maxRegisters,
            returnArity = returnArity,
            upvalues = upvalues,
            children = children,
            localNames = localNames,
            constants = constants,
            instructions = instructions,
        }
    end
    return {
        magic = magic,
        version = version,
        featureFlags = featureFlags,
        entry = entry,
        prototypes = prototypes,
    }
end
"#
}
