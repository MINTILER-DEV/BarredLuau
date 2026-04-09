use barred_luau::parser::{AstBackend, Expression, MockLuauBackend, Statement};

#[test]
fn parser_accepts_hex_and_binary_literals() {
    let backend = MockLuauBackend;
    let ast = backend
        .parse(
            r#"
local a = 0xFFFF
local b = 0b11010
"#,
        )
        .expect("parse");

    let Statement::LocalDeclaration { values, .. } = &ast.block.statements[0] else {
        panic!("expected first statement to be a local declaration");
    };
    let Statement::LocalDeclaration {
        values: second_values,
        ..
    } = &ast.block.statements[1]
    else {
        panic!("expected second statement to be a local declaration");
    };

    assert!(matches!(values[0], Expression::Number(value) if value == 65535.0));
    assert!(matches!(second_values[0], Expression::Number(value) if value == 26.0));
}
