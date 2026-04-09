use std::fmt::Write;

use crate::config::{BuildMode, CompileConfig};
use crate::ir::{OpcodeRegistry, ProgramBlob};
use crate::obfuscation::anti_tamper::RuntimeIntegrity;
use crate::obfuscation::identifier_mangler::IdentifierMangler;
use crate::obfuscation::literal_encoder::{
    emit_luau_encoded_string_expr, emit_luau_xor_decoder, xor_string_literal,
};
use crate::serializer::EncoderKey;
use crate::vmgen::dispatcher_template::emit_dispatcher;
use crate::vmgen::luau_runtime_template::emit_runtime_support;
use crate::vmgen::opcode_handler_template::emit_opcode_constants;

const GENERATED_COMMENT: &str = "-- generated with BarredLuau";
const INTEGRITY_ERROR_LABEL: &str = "barx1";
const RUNTIME_FAULT_LABEL: &str = "barx2";

#[derive(Clone, Debug)]
struct RuntimeStringPool {
    decoder_name: String,
    getter_name: String,
    table_name: String,
    key_seed: u8,
    entries: Vec<(u8, Vec<u8>)>,
    reverse: std::collections::BTreeMap<String, usize>,
}

impl RuntimeStringPool {
    fn new(decoder_name: String, getter_name: String, table_name: String, key_seed: u8) -> Self {
        Self {
            decoder_name,
            getter_name,
            table_name,
            key_seed,
            entries: Vec::new(),
            reverse: std::collections::BTreeMap::new(),
        }
    }

    fn expr(&mut self, value: &str) -> String {
        if let Some(index) = self.reverse.get(value) {
            return format!("{}({})", self.getter_name, index + 1);
        }

        let key = self
            .key_seed
            .wrapping_add((self.entries.len() as u8).wrapping_mul(29))
            .wrapping_add(value.len() as u8);
        let encoded = xor_string_literal(value, key);
        let index = self.entries.len();
        self.entries.push((key, encoded));
        self.reverse.insert(value.to_string(), index);
        format!("{}({})", self.getter_name, index + 1)
    }

    fn emit_prelude(&self) -> String {
        let mut out = String::new();
        let _ = write!(out, "local {}={{", self.table_name);
        for (entry_index, (key, bytes)) in self.entries.iter().enumerate() {
            if entry_index > 0 {
                out.push(',');
            }
            out.push('{');
            let _ = write!(out, "{key},{{");
            for (byte_index, byte) in bytes.iter().enumerate() {
                if byte_index > 0 {
                    out.push(',');
                }
                let _ = write!(out, "{byte}");
            }
            out.push_str("}}");
        }
        out.push_str("}\n");
        let _ = writeln!(
            out,
            "local {}=function(i)local e={}[i]return {}(e[2],e[1])end",
            self.getter_name, self.table_name, self.decoder_name
        );
        out
    }
}

pub fn emit_luau_output(
    program: &ProgramBlob,
    encoded_blob: &str,
    key: &EncoderKey,
    config: &CompileConfig,
    registry: &OpcodeRegistry,
    integrity: &RuntimeIntegrity,
) -> String {
    let readable = matches!(config.mode, BuildMode::Debug);
    let use_string_pool = !readable && config.obfuscation.pool_runtime_strings;
    let use_handler_indirection = !readable && config.obfuscation.handler_indirection;
    let mut mangler = IdentifierMangler::new(config.seed);
    let string_decoder_name = if readable {
        "decodeRuntimeString".to_string()
    } else {
        mangler.next_ident()
    };
    let encoded_name = if readable {
        "encodedBlob".to_string()
    } else {
        mangler.next_ident()
    };
    let key_name = if readable {
        "runtimeKey".to_string()
    } else {
        mangler.next_ident()
    };
    let cfg_name = if readable {
        "runtimeCfg".to_string()
    } else {
        mangler.next_ident()
    };
    let program_name = if readable {
        "program".to_string()
    } else {
        mangler.next_ident()
    };
    let output_name = if readable {
        "bootstrap".to_string()
    } else {
        mangler.next_ident()
    };
    let runtime_string_key = config.seed.wrapping_mul(31).wrapping_add(17) as u8;
    let mut runtime_pool = if use_string_pool {
        Some(RuntimeStringPool::new(
            string_decoder_name.clone(),
            mangler.next_ident(),
            mangler.next_ident(),
            runtime_string_key,
        ))
    } else {
        None
    };
    let alphabet_expr = runtime_string_expr(
        readable,
        &mut runtime_pool,
        &config.encoder.alphabet,
        runtime_string_key,
        &string_decoder_name,
    );
    let magic_expr = runtime_string_expr(
        readable,
        &mut runtime_pool,
        "BRLU",
        runtime_string_key,
        &string_decoder_name,
    );
    let integrity_error_expr = format!("\"{INTEGRITY_ERROR_LABEL}\"");
    let runtime_fault_expr = format!("\"{RUNTIME_FAULT_LABEL}\"");
    let colon_expr = runtime_string_expr(
        readable,
        &mut runtime_pool,
        ":",
        runtime_string_key,
        &string_decoder_name,
    );
    let unpack_i4_expr = runtime_string_expr(
        readable,
        &mut runtime_pool,
        "<i4",
        runtime_string_key,
        &string_decoder_name,
    );
    let unpack_d_expr = runtime_string_expr(
        readable,
        &mut runtime_pool,
        "<d",
        runtime_string_key,
        &string_decoder_name,
    );

    let mut out = String::new();
    if readable {
        let _ = writeln!(out, "{GENERATED_COMMENT}");
    }
    if !readable {
        out.push_str(&emit_luau_xor_decoder(&string_decoder_name));
        if let Some(pool) = runtime_pool.as_ref() {
            out.push_str(&pool.emit_prelude());
        }
    }
    let _ = writeln!(out, "local {encoded_name} = \"{encoded_blob}\"");
    let _ = writeln!(
        out,
        "local {key_name} = {{ seed = {}, nonce = {} }}",
        key.seed, key.nonce
    );
    let _ = writeln!(
        out,
        "local {cfg_name} = {{ rounds = {}, alphabet = {}, chunkSize = {}, includeChecksum = {}, interleave = {} }}",
        config.encoder.rounds,
        alphabet_expr,
        config.encoder.chunk_size,
        if config.encoder.include_checksum {
            "true"
        } else {
            "false"
        },
        if config.encoder.interleave {
            "true"
        } else {
            "false"
        }
    );
    out.push_str(&emit_opcode_constants(registry));
    out.push_str(emit_runtime_support());
    out.push_str(&emit_dispatcher(use_handler_indirection));
    let _ = writeln!(out, "local function {output_name}()");
    let _ = writeln!(out, "    local runtimeEnv = (getfenv and getfenv()) or _G");
    let _ = writeln!(
        out,
        "    local decoded = decodePayload({encoded_name}, {key_name}, {cfg_name})"
    );
    let _ = writeln!(
        out,
        "    local {program_name} = deserializeProgram(decoded)"
    );
    if config.anti_tamper.enabled {
        let _ = writeln!(
            out,
            "    if {program_name}.magic ~= {magic_expr} or {program_name}.version ~= {} then error({integrity_error_expr}) end",
            program.version,
        );
        if integrity.verify_opcode_table {
            let _ = writeln!(
                out,
                "    if {} ~= {} then error({integrity_error_expr}) end",
                registry.checksum(),
                integrity.opcode_checksum
            );
        }
    }
    let _ = writeln!(
        out,
        "    return executeProto({program_name}.prototypes[{program_name}.entry + 1], {program_name}.prototypes, runtimeEnv, {{}}, {{}}, {{}})"
    );
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "return {output_name}()");

    if readable {
        out
    } else {
        let obfuscated = obfuscate_bootstrap_identifiers(&out, &mut mangler);
        let obfuscated = obfuscated.replace(
            "\"barredluau integrity check failed\"",
            &integrity_error_expr,
        );
        let obfuscated = obfuscated.replace("\"barredluau runtime fault\"", &runtime_fault_expr);
        let obfuscated = obfuscated.replace("\"BRLU\"", &magic_expr);
        let obfuscated = obfuscated.replace("\":\"", &colon_expr);
        let obfuscated = obfuscated.replace("\"<i4\"", &unpack_i4_expr);
        let obfuscated = obfuscated.replace("\"<d\"", &unpack_d_expr);
        format!("{GENERATED_COMMENT}\n{}", minify_luau(&obfuscated))
    }
}

fn obfuscate_bootstrap_identifiers(source: &str, mangler: &mut IdentifierMangler) -> String {
    let identifiers = [
        "OPCODES",
        "DISPATCH",
        "LoadNil",
        "LoadBool",
        "LoadNumber",
        "LoadString",
        "Move",
        "GetGlobal",
        "SetGlobal",
        "GetTable",
        "SetTable",
        "NewTable",
        "Call",
        "CallSpread",
        "Return",
        "ReturnSpread",
        "Jump",
        "JumpIf",
        "JumpIfNot",
        "Closure",
        "GetUpvalue",
        "SetUpvalue",
        "Concat",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Mod",
        "Pow",
        "Eq",
        "Lt",
        "Le",
        "Len",
        "Not",
        "fnv32",
        "mul32",
        "add32",
        "fail",
        "readPackedU32",
        "newPrng",
        "buildPermutation",
        "invertPermutation",
        "streamTransform",
        "buildSubstitution",
        "invertSubstitution",
        "substitute",
        "decodeText",
        "deinterleave",
        "bytesToString",
        "decodePayload",
        "deserializeProgram",
        "encodedBlob",
        "runtimeKey",
        "runtimeCfg",
        "program",
        "runtimeEnv",
        "readOperand",
        "writeRegister",
        "captureClosure",
        "executeProto",
        "returnState",
        "shouldReturn",
        "handler",
        "instruction",
        "operand",
        "digit",
        "bytes",
        "bound",
        "state",
        "nextU32",
        "nextByte",
        "bounded",
        "length",
        "seed",
        "nonce",
        "rounds",
        "alphabet",
        "chunkSize",
        "includeChecksum",
        "interleave",
        "permutation",
        "prng",
        "target",
        "tmp",
        "restored",
        "substitution",
        "inverse",
        "text",
        "reverse",
        "digits",
        "radix",
        "leftSize",
        "parts",
        "offset",
        "chunk",
        "upper",
        "payload",
        "round",
        "roundSeed",
        "start",
        "checksum",
        "data",
        "blob",
        "cursor",
        "readByte",
        "readU16",
        "readU32",
        "readVarU32",
        "readString",
        "magic",
        "version",
        "featureFlags",
        "entry",
        "protoCount",
        "prototypes",
        "protoIndex",
        "hasName",
        "name",
        "paramCount",
        "params",
        "isVararg",
        "varargRegister",
        "maxRegisters",
        "returnArity",
        "upvalueCount",
        "upvalues",
        "childCount",
        "children",
        "localNameCount",
        "localNames",
        "constantCount",
        "constants",
        "instructionCount",
        "instructions",
        "tag",
        "op",
        "value",
        "nextCursor",
        "frame",
        "registers",
        "namedLocals",
        "registerIndex",
        "captured",
        "upvalueMap",
        "proto",
        "env",
        "args",
        "index",
        "outputIndex",
        "argIndex",
        "base",
        "fixedCount",
        "callee",
        "argsBuffer",
        "spread",
        "spreadIndex",
        "count",
        "values",
        "resultIndex",
        "child",
        "tbl",
        "key",
        "results",
        "rhs",
        "decoded",
        "hash",
        "tableValues",
        "mask",
        "encode",
        "cell",
        "pc",
        "packed",
        "a",
        "b",
        "c",
        "d",
        "o",
    ];

    let mut map = std::collections::BTreeMap::new();
    for identifier in identifiers {
        map.insert(identifier, mangler.next_ident());
    }
    replace_identifiers(source, &map)
}

fn runtime_string_expr(
    readable: bool,
    pool: &mut Option<RuntimeStringPool>,
    value: &str,
    key: u8,
    decoder_name: &str,
) -> String {
    if readable {
        format!("\"{value}\"")
    } else if let Some(pool) = pool.as_mut() {
        pool.expr(value)
    } else {
        emit_luau_encoded_string_expr(value, key, decoder_name)
    }
}

fn replace_identifiers(
    source: &str,
    replacements: &std::collections::BTreeMap<&str, String>,
) -> String {
    let mut out = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut quote: Option<char> = None;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(delimiter) = quote {
            out.push(ch);
            if ch == '\\' {
                index += 1;
                if index < chars.len() {
                    out.push(chars[index]);
                }
            } else if ch == delimiter {
                quote = None;
            }
            index += 1;
            continue;
        }

        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            out.push(ch);
            index += 1;
            continue;
        }

        if is_ident_start(ch) {
            let start = index;
            index += 1;
            while index < chars.len() && is_ident_continue(chars[index]) {
                index += 1;
            }
            let ident: String = chars[start..index].iter().collect();
            if let Some(replacement) = replacements.get(ident.as_str()) {
                out.push_str(replacement);
            } else {
                out.push_str(&ident);
            }
            continue;
        }

        out.push(ch);
        index += 1;
    }
    out
}

fn minify_luau(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0usize;
    let mut quote: Option<char> = None;
    let mut pending_space = false;

    while index < chars.len() {
        let ch = chars[index];
        if let Some(delimiter) = quote {
            out.push(ch);
            if ch == '\\' {
                index += 1;
                if index < chars.len() {
                    out.push(chars[index]);
                }
            } else if ch == delimiter {
                quote = None;
            }
            index += 1;
            continue;
        }

        if ch == '\'' || ch == '"' {
            if pending_space && needs_space(out.chars().last(), Some(ch)) {
                out.push(' ');
            }
            pending_space = false;
            quote = Some(ch);
            out.push(ch);
            index += 1;
            continue;
        }

        if ch == '-' && chars.get(index + 1) == Some(&'-') {
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            pending_space = true;
            continue;
        }

        if ch.is_whitespace() {
            pending_space = true;
            index += 1;
            continue;
        }

        if pending_space && needs_space(out.chars().last(), Some(ch)) {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
        index += 1;
    }

    out
}

fn needs_space(prev: Option<char>, next: Option<char>) -> bool {
    match (prev, next) {
        (Some(prev), Some(next))
            if is_token_char(prev) && (is_token_char(next) || is_quote(next)) =>
        {
            true
        }
        (Some(prev), Some(next)) if is_quote(prev) && is_token_char(next) => true,
        _ => false,
    }
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_quote(ch: char) -> bool {
    ch == '\'' || ch == '"'
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
