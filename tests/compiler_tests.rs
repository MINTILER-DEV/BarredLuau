#[path = "support/sample_programs.rs"]
mod sample_programs;

use barred_luau::compiler::compile_program_to_ir;
use barred_luau::config::CompileConfig;
use barred_luau::ir::Opcode;
use barred_luau::parser::scope_analyzer::analyze_program;
use barred_luau::parser::{AstBackend, MockLuauBackend};

#[test]
fn compiler_emits_arithmetic_and_table_ops() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(sample_programs::arithmetic_and_tables())
        .expect("parse");
    let ir = compile_program_to_ir(&ast, &CompileConfig::default()).expect("compile");
    let opcodes: Vec<Opcode> = ir.prototypes[0]
        .instructions
        .iter()
        .map(|instruction| instruction.opcode)
        .collect();
    assert!(opcodes.contains(&Opcode::Add));
    assert!(opcodes.contains(&Opcode::NewTable));
    assert!(opcodes.contains(&Opcode::SetTable));
}

#[test]
fn scope_analysis_detects_upvalues() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(sample_programs::closure_capture())
        .expect("parse");
    let summary = analyze_program(&ast).expect("scope analysis");
    assert!(
        summary
            .functions
            .iter()
            .any(|function| function.upvalues.contains("seed"))
    );
}

#[test]
fn compiler_emits_closure_opcode() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(sample_programs::closure_capture())
        .expect("parse");
    let ir = compile_program_to_ir(&ast, &CompileConfig::default()).expect("compile");
    let contains_closure = ir
        .prototypes
        .iter()
        .flat_map(|proto| proto.instructions.iter())
        .any(|instruction| instruction.opcode == Opcode::Closure);
    assert!(contains_closure);
}

#[test]
fn compiler_lifts_transitive_upvalues_for_nested_closures() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(sample_programs::closure_capture())
        .expect("parse");
    let ir = compile_program_to_ir(&ast, &CompileConfig::default()).expect("compile");

    let make_adder = ir
        .prototypes
        .iter()
        .find(|proto| proto.name.as_deref() == Some("makeAdder"))
        .expect("makeAdder proto");
    let inner = ir
        .prototypes
        .iter()
        .find(|proto| proto.name.is_none() && proto.parameters == vec!["y".to_string()])
        .expect("inner closure proto");

    assert!(make_adder.upvalues.contains(&"seed".to_string()));
    assert!(inner.upvalues.contains(&"x".to_string()));
    assert!(inner.upvalues.contains(&"seed".to_string()));
}

#[test]
fn compiler_accepts_method_call_syntax() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(sample_programs::method_calls())
        .expect("parse");
    let ir = compile_program_to_ir(&ast, &CompileConfig::default()).expect("compile");
    let opcodes: Vec<Opcode> = ir
        .prototypes
        .iter()
        .flat_map(|proto| proto.instructions.iter())
        .map(|instruction| instruction.opcode)
        .collect();

    assert!(opcodes.contains(&Opcode::GetTable));
    assert!(opcodes.contains(&Opcode::Call));
}

#[test]
fn compiler_supports_varargs_short_circuit_and_for_loops() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(
            r#"
local function wrap(...)
    local args = {...}
    local total = 0
    local function addPair(a, b)
        return a + b
    end
    for i = 1, #args do
        total += args[i]
    end
    for index, value in ipairs(args) do
        if index == 1 or value > 100 then
            total = total and total or 0
        end
    end
    total = addPair(table.unpack(args))
    return table.unpack(args)
end
return wrap(3, 4)
"#,
        )
        .expect("parse");
    let ir = compile_program_to_ir(&ast, &CompileConfig::default()).expect("compile");
    let opcodes: Vec<Opcode> = ir
        .prototypes
        .iter()
        .flat_map(|proto| proto.instructions.iter())
        .map(|instruction| instruction.opcode)
        .collect();

    assert!(opcodes.contains(&Opcode::JumpIf));
    assert!(opcodes.contains(&Opcode::JumpIfNot));
    assert!(opcodes.contains(&Opcode::CallSpread));
    assert!(opcodes.contains(&Opcode::ReturnSpread));
}
