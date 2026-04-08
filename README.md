# BarredLuau

`BarredLuau` is a local Rust toolchain for protecting Roblox Luau source through a custom register-based IR, binary blob serialization, a reversible non-Base64 encoder, and emitted Luau VM runtime scaffolding.

## Architecture

The pipeline is intentionally modular:

1. `parser/`
   - A parser abstraction plus a strict mock Luau backend for a useful Luau subset.
   - AST traversal helpers and scope analysis for locals, globals, parameters, returns, and upvalues.
2. `ir/`
   - A typed register-based IR with function prototypes, constant pools, and an opcode registry that can be randomized per build.
3. `compiler/`
   - AST to IR lowering for core statements and expressions.
   - Unsupported syntax returns structured errors instead of silently degrading behavior.
4. `serializer/`
   - Compact binary blob serialization with header, version, feature flags, prototypes, instructions, and checksum sealing.
5. `serializer/custom_encoder.rs`
   - Deterministic layered encoding using permutation, stream transforms, substitution tables, and a custom radix alphabet.
   - No Base64 or JSON payload encoding is used.
6. `vmgen/`
   - Rust-side Luau source emission for decoder, deserializer, dispatcher, closure capture, and bootstrap logic.
7. `obfuscation/`
   - Lightweight anti-tamper metadata, runtime identifier mangling, literal helpers, and string pooling primitives.
8. `pipeline/`
   - End-to-end compile path exposed as `pub fn compile(source: &str, config: &CompileConfig) -> Result<String, CompileError>`.

## Current Support

Implemented and compiled today:

- Local declarations
- Assignments
- Numeric, string, boolean, and nil literals
- Variable references
- Function declarations and anonymous functions
- Function calls
- `if / elseif / else`
- `while`
- `repeat / until`
- Returns
- Arithmetic, comparison, concat, `not`, and `#`
- Table constructors and indexing
- Closure capture analysis and IR closure emission

Parsed but not yet lowered:

- Numeric `for`
- Generic `for`
- Short-circuit `and` / `or`
- Dotted or method-style function declarations

Those cases fail with explicit structured errors so future compiler passes can expand coverage safely.

## CLI

```bash
cargo run -- \
  --input examples/sample_input.luau \
  --output examples/sample_output.protected.luau \
  --release \
  --anti-tamper \
  --randomize-opcodes \
  --seed 1337 \
  --encoder-rounds 3 \
  --target roblox-luau
```

## Tests

```bash
cargo test
```

The integration suite covers encoder roundtrips, checksum failure detection, serializer roundtrips, compiler opcode emission, scope analysis, and emitted runtime shape.

## Notes

- The compiler is written entirely in Rust and emits valid Luau source for Roblox consumption.
- The custom encoder is reversible and deterministic, but intentionally nonstandard.
- Decoder and deserializer remain separate so later hardening can swap or layer them independently.
- Future optimization hooks are reserved for peephole optimization, instruction fusion, selective virtualization, and per-function protection modes.
