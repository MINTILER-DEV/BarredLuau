use barred_luau::compiler::compile_program_to_ir;
use barred_luau::config::CompileConfig;
use barred_luau::ir::{ConstantValue, Opcode, Operand, ProgramBlob, PrototypeId};
use barred_luau::parser::{AstBackend, MockLuauBackend};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

type VmResult<T> = Result<T, String>;
type Cell = Rc<RefCell<Value>>;
type NativeFn = Rc<dyn Fn(Vec<Value>) -> VmResult<Vec<Value>>>;
type Globals = Rc<RefCell<BTreeMap<String, Value>>>;

#[derive(Clone)]
enum Value {
    Nil,
    Boolean(bool),
    Number(f64),
    String(String),
    Table(Rc<RefCell<BTreeMap<TableKey, Value>>>),
    NativeFunction(NativeFn),
    Closure(Rc<ClosureValue>),
}

#[derive(Clone)]
struct ClosureValue {
    program: Rc<ProgramBlob>,
    globals: Globals,
    proto_id: PrototypeId,
    upvalues: Vec<Cell>,
    upvalue_map: BTreeMap<String, Cell>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TableKey {
    Integer(i64),
    String(String),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Boolean(value) => write!(f, "{value}"),
            Value::Number(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value:?}"),
            Value::Table(_) => write!(f, "<table>"),
            Value::NativeFunction(_) => write!(f, "<native>"),
            Value::Closure(_) => write!(f, "<closure>"),
        }
    }
}

impl Value {
    fn expect_string(&self) -> VmResult<&str> {
        match self {
            Value::String(value) => Ok(value),
            other => Err(format!("expected string, got {other:?}")),
        }
    }

    fn expect_number(&self) -> VmResult<f64> {
        match self {
            Value::Number(value) => Ok(*value),
            other => Err(format!("expected number, got {other:?}")),
        }
    }

    fn expect_table(&self) -> VmResult<Rc<RefCell<BTreeMap<TableKey, Value>>>> {
        match self {
            Value::Table(table) => Ok(table.clone()),
            other => Err(format!("expected table, got {other:?}")),
        }
    }
}

struct Frame {
    program: Rc<ProgramBlob>,
    proto_id: PrototypeId,
    globals: Globals,
    upvalues: Vec<Cell>,
    upvalue_map: BTreeMap<String, Cell>,
    named_locals: BTreeMap<String, Cell>,
    registers: Vec<Cell>,
    pc: usize,
}

fn compile_source(source: &str) -> ProgramBlob {
    let backend = MockLuauBackend;
    let ast = backend.parse(source).expect("parse");
    compile_program_to_ir(&ast, &CompileConfig::default()).expect("compile")
}

fn encode_blob(bytes: &[u8], alphabet: &str) -> String {
    let alphabet: Vec<char> = alphabet.chars().collect();
    let radix = alphabet.len();
    let mut encoded = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        let high = usize::from(*byte) / radix;
        let low = usize::from(*byte) % radix;
        encoded.push(alphabet[high]);
        encoded.push(alphabet[low]);
        if index == 1 {
            encoded.push(':');
        }
    }
    encoded
}

fn globals() -> Globals {
    fn native(
        name: &'static str,
        f: impl Fn(Vec<Value>) -> VmResult<Vec<Value>> + 'static,
    ) -> Value {
        let _ = name;
        Value::NativeFunction(Rc::new(f))
    }

    let globals = Rc::new(RefCell::new(BTreeMap::new()));

    let string_table = Rc::new(RefCell::new(BTreeMap::new()));
    string_table.borrow_mut().insert(
        TableKey::String("char".to_string()),
        native("string.char", |args| {
            let mut out = String::new();
            for arg in args {
                let code = arg.expect_number()? as u32;
                let ch = char::from_u32(code).ok_or_else(|| format!("invalid char code {code}"))?;
                out.push(ch);
            }
            Ok(vec![Value::String(out)])
        }),
    );
    string_table.borrow_mut().insert(
        TableKey::String("sub".to_string()),
        native("string.sub", |args| {
            let source = args[0].expect_string()?.chars().collect::<Vec<_>>();
            let start = args[1].expect_number()? as usize;
            let end = args[2].expect_number()? as usize;
            let slice = if start == 0 || end < start {
                String::new()
            } else {
                source[start - 1..end.min(source.len())].iter().collect()
            };
            Ok(vec![Value::String(slice)])
        }),
    );
    string_table.borrow_mut().insert(
        TableKey::String("byte".to_string()),
        native("string.byte", |args| {
            let source = args[0].expect_string()?.chars().collect::<Vec<_>>();
            let start = args
                .get(1)
                .map(Value::expect_number)
                .transpose()?
                .unwrap_or(1.0) as usize;
            let end = args
                .get(2)
                .map(Value::expect_number)
                .transpose()?
                .unwrap_or(start as f64) as usize;
            let mut values = Vec::new();
            if start == 0 || start > source.len() {
                return Ok(values);
            }
            for index in start - 1..end.min(source.len()) {
                values.push(Value::Number(source[index] as u32 as f64));
            }
            Ok(values)
        }),
    );

    let table_table = Rc::new(RefCell::new(BTreeMap::new()));
    table_table.borrow_mut().insert(
        TableKey::String("concat".to_string()),
        native("table.concat", |args| {
            let table = args[0].expect_table()?;
            let len = table_len(&table.borrow());
            let mut out = String::new();
            for index in 1..=len {
                let value = table
                    .borrow()
                    .get(&TableKey::Integer(index as i64))
                    .cloned()
                    .unwrap_or(Value::Nil);
                out.push_str(value.expect_string()?);
            }
            Ok(vec![Value::String(out)])
        }),
    );
    table_table.borrow_mut().insert(
        TableKey::String("create".to_string()),
        native("table.create", |args| {
            let len = args[0].expect_number()? as usize;
            let fill = args.get(1).cloned().unwrap_or(Value::Nil);
            let table = Rc::new(RefCell::new(BTreeMap::new()));
            for index in 1..=len {
                table
                    .borrow_mut()
                    .insert(TableKey::Integer(index as i64), fill.clone());
            }
            Ok(vec![Value::Table(table)])
        }),
    );

    let bit32_table = Rc::new(RefCell::new(BTreeMap::new()));
    bit32_table.borrow_mut().insert(
        TableKey::String("bxor".to_string()),
        native("bit32.bxor", |args| {
            let lhs = args[0].expect_number()? as u32;
            let rhs = args[1].expect_number()? as u32;
            Ok(vec![Value::Number(f64::from(lhs ^ rhs))])
        }),
    );
    bit32_table.borrow_mut().insert(
        TableKey::String("band".to_string()),
        native("bit32.band", |args| {
            let lhs = args[0].expect_number()? as u32;
            let rhs = args[1].expect_number()? as u32;
            Ok(vec![Value::Number(f64::from(lhs & rhs))])
        }),
    );
    bit32_table.borrow_mut().insert(
        TableKey::String("rshift".to_string()),
        native("bit32.rshift", |args| {
            let lhs = args[0].expect_number()? as u32;
            let rhs = args[1].expect_number()? as u32;
            Ok(vec![Value::Number(f64::from(lhs >> rhs))])
        }),
    );
    bit32_table.borrow_mut().insert(
        TableKey::String("lshift".to_string()),
        native("bit32.lshift", |args| {
            let lhs = args[0].expect_number()? as u32;
            let rhs = args[1].expect_number()? as u32;
            Ok(vec![Value::Number(f64::from(lhs << rhs))])
        }),
    );
    bit32_table.borrow_mut().insert(
        TableKey::String("lrotate".to_string()),
        native("bit32.lrotate", |args| {
            let value = args[0].expect_number()? as u32;
            let shift = (args[1].expect_number()? as u32) & 31;
            Ok(vec![Value::Number(f64::from(value.rotate_left(shift)))])
        }),
    );

    let math_table = Rc::new(RefCell::new(BTreeMap::new()));
    math_table.borrow_mut().insert(
        TableKey::String("ceil".to_string()),
        native("math.ceil", |args| {
            Ok(vec![Value::Number(args[0].expect_number()?.ceil())])
        }),
    );
    math_table.borrow_mut().insert(
        TableKey::String("min".to_string()),
        native("math.min", |args| {
            let lhs = args[0].expect_number()?;
            let rhs = args[1].expect_number()?;
            Ok(vec![Value::Number(lhs.min(rhs))])
        }),
    );

    let env_table = Rc::new(RefCell::new(BTreeMap::new()));

    let mut env = globals.borrow_mut();
    env.insert("string".to_string(), Value::Table(string_table));
    env.insert("table".to_string(), Value::Table(table_table));
    env.insert("bit32".to_string(), Value::Table(bit32_table));
    env.insert("math".to_string(), Value::Table(math_table));
    env.insert(
        "error".to_string(),
        native("error", |args| {
            let message = args
                .first()
                .map(value_to_string)
                .unwrap_or_else(|| "error".to_string());
            Err(message)
        }),
    );
    env.insert("_G".to_string(), Value::Table(env_table.clone()));
    env_table.borrow_mut().insert(
        TableKey::String("string".to_string()),
        env.get("string").cloned().unwrap(),
    );
    env_table.borrow_mut().insert(
        TableKey::String("table".to_string()),
        env.get("table").cloned().unwrap(),
    );
    env_table.borrow_mut().insert(
        TableKey::String("bit32".to_string()),
        env.get("bit32").cloned().unwrap(),
    );
    env_table.borrow_mut().insert(
        TableKey::String("math".to_string()),
        env.get("math").cloned().unwrap(),
    );
    drop(env);
    globals
}

fn table_len(table: &BTreeMap<TableKey, Value>) -> usize {
    let mut len = 0usize;
    loop {
        let next = len + 1;
        if table.contains_key(&TableKey::Integer(next as i64)) {
            len = next;
        } else {
            return len;
        }
    }
}

fn operand_value(frame: &Frame, operand: &Operand) -> VmResult<Value> {
    match operand {
        Operand::None => Ok(Value::Nil),
        Operand::Register(register) => Ok(frame.registers[register.0 as usize].borrow().clone()),
        Operand::Constant(index) => match &frame.proto().constants.values[index.0 as usize] {
            ConstantValue::Nil => Ok(Value::Nil),
            ConstantValue::Boolean(value) => Ok(Value::Boolean(*value)),
            ConstantValue::Number(value) => Ok(Value::Number(*value)),
            ConstantValue::String(value) => Ok(Value::String(value.clone())),
        },
        Operand::Immediate(value) => Ok(Value::Number(f64::from(*value))),
        Operand::Upvalue(index) => Ok(frame.upvalues[*index as usize].borrow().clone()),
        Operand::Boolean(value) => Ok(Value::Boolean(*value)),
        Operand::Prototype(_) => Err("prototype operand cannot be read directly".to_string()),
    }
}

impl Frame {
    fn proto(&self) -> &barred_luau::ir::FunctionProto {
        &self.program.prototypes[self.proto_id.0 as usize]
    }
}

fn capture_closure(frame: &Frame, proto_id: PrototypeId) -> Value {
    let proto = &frame.program.prototypes[proto_id.0 as usize];
    let mut captured = Vec::with_capacity(proto.upvalues.len());
    let mut upvalue_map = BTreeMap::new();
    for name in &proto.upvalues {
        let cell = frame
            .named_locals
            .get(name.as_str())
            .cloned()
            .or_else(|| frame.upvalue_map.get(name.as_str()).cloned())
            .unwrap_or_else(|| {
                let value = frame
                    .globals
                    .borrow()
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or(Value::Nil);
                Rc::new(RefCell::new(value))
            });
        captured.push(cell.clone());
        upvalue_map.insert(name.clone(), cell);
    }
    Value::Closure(Rc::new(ClosureValue {
        program: frame.program.clone(),
        globals: frame.globals.clone(),
        proto_id,
        upvalues: captured,
        upvalue_map,
    }))
}

fn call_value(callee: Value, args: Vec<Value>) -> VmResult<Vec<Value>> {
    match callee {
        Value::NativeFunction(function) => function(args),
        Value::Closure(closure) => execute_proto(
            closure.program.clone(),
            closure.globals.clone(),
            closure.proto_id,
            closure.upvalues.clone(),
            closure.upvalue_map.clone(),
            args,
        ),
        other => Err(format!("cannot call {other:?}")),
    }
}

fn execute_proto(
    program: Rc<ProgramBlob>,
    globals: Globals,
    proto_id: PrototypeId,
    upvalues: Vec<Cell>,
    upvalue_map: BTreeMap<String, Cell>,
    args: Vec<Value>,
) -> VmResult<Vec<Value>> {
    let proto = &program.prototypes[proto_id.0 as usize];
    let mut registers = Vec::with_capacity(proto.max_registers as usize);
    for _ in 0..proto.max_registers {
        registers.push(Rc::new(RefCell::new(Value::Nil)));
    }
    for (index, value) in args.into_iter().enumerate() {
        if let Some(register) = registers.get(index) {
            *register.borrow_mut() = value;
        }
    }
    let mut named_locals = BTreeMap::new();
    for (index, name) in proto.local_names.iter().enumerate() {
        if let Some(name) = name {
            named_locals.insert(name.clone(), registers[index].clone());
        }
    }
    let mut frame = Frame {
        program,
        proto_id,
        globals,
        upvalues,
        upvalue_map,
        named_locals,
        registers,
        pc: 0,
    };

    loop {
        let instruction = frame
            .proto()
            .instructions
            .get(frame.pc)
            .cloned()
            .ok_or_else(|| "ran off the end of the instruction stream".to_string())?;
        frame.pc += 1;

        match instruction.opcode {
            Opcode::LoadNil => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("LoadNil dst".to_string());
                };
                *frame.registers[dst.0 as usize].borrow_mut() = Value::Nil;
            }
            Opcode::LoadBool => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("LoadBool dst".to_string());
                };
                let Operand::Boolean(value) = instruction.b else {
                    return Err("LoadBool src".to_string());
                };
                *frame.registers[dst.0 as usize].borrow_mut() = Value::Boolean(value);
            }
            Opcode::LoadNumber | Opcode::LoadString | Opcode::Move => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("copy dst".to_string());
                };
                *frame.registers[dst.0 as usize].borrow_mut() =
                    operand_value(&frame, &instruction.b)?;
            }
            Opcode::GetGlobal => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("GetGlobal dst".to_string());
                };
                let name = operand_value(&frame, &instruction.b)?
                    .expect_string()?
                    .to_string();
                let value = frame
                    .globals
                    .borrow()
                    .get(&name)
                    .cloned()
                    .unwrap_or(Value::Nil);
                *frame.registers[dst.0 as usize].borrow_mut() = value;
            }
            Opcode::SetGlobal => {
                let name = operand_value(&frame, &instruction.a)?
                    .expect_string()?
                    .to_string();
                let value = operand_value(&frame, &instruction.b)?;
                frame.globals.borrow_mut().insert(name, value);
            }
            Opcode::NewTable => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("NewTable dst".to_string());
                };
                *frame.registers[dst.0 as usize].borrow_mut() =
                    Value::Table(Rc::new(RefCell::new(BTreeMap::new())));
            }
            Opcode::GetTable => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("GetTable dst".to_string());
                };
                let table = operand_value(&frame, &instruction.b)?.expect_table()?;
                let key = table_key(&operand_value(&frame, &instruction.c)?)?;
                let value = table.borrow().get(&key).cloned().unwrap_or(Value::Nil);
                *frame.registers[dst.0 as usize].borrow_mut() = value;
            }
            Opcode::SetTable => {
                let table = operand_value(&frame, &instruction.a)?.expect_table()?;
                let key = table_key(&operand_value(&frame, &instruction.b)?)?;
                let value = operand_value(&frame, &instruction.c)?;
                table.borrow_mut().insert(key, value);
            }
            Opcode::Call => {
                let Operand::Register(base) = instruction.a else {
                    return Err("Call base".to_string());
                };
                let Operand::Immediate(arg_count) = instruction.b else {
                    return Err("Call arg count".to_string());
                };
                let Operand::Register(dst) = instruction.c else {
                    return Err("Call dst".to_string());
                };
                let base = base.0 as usize;
                let callee = frame.registers[base].borrow().clone();
                let mut args = Vec::with_capacity(arg_count as usize);
                for offset in 0..arg_count as usize {
                    args.push(frame.registers[base + offset + 1].borrow().clone());
                }
                let result = call_value(callee, args)?
                    .into_iter()
                    .next()
                    .unwrap_or(Value::Nil);
                *frame.registers[dst.0 as usize].borrow_mut() = result;
            }
            Opcode::Return => {
                let Operand::Register(base) = instruction.a else {
                    return Err("Return base".to_string());
                };
                let Operand::Immediate(count) = instruction.b else {
                    return Err("Return count".to_string());
                };
                let mut values = Vec::with_capacity(count as usize);
                for offset in 0..count as usize {
                    values.push(frame.registers[base.0 as usize + offset].borrow().clone());
                }
                return Ok(values);
            }
            Opcode::Jump => {
                let Operand::Immediate(offset) = instruction.b else {
                    return Err("Jump offset".to_string());
                };
                frame.pc = ((frame.pc as isize) + (offset as isize)) as usize;
            }
            Opcode::JumpIf | Opcode::JumpIfNot => {
                let value = operand_value(&frame, &instruction.a)?;
                let condition = !matches!(value, Value::Nil | Value::Boolean(false));
                let should_jump = match instruction.opcode {
                    Opcode::JumpIf => condition,
                    Opcode::JumpIfNot => !condition,
                    _ => unreachable!(),
                };
                if should_jump {
                    let Operand::Immediate(offset) = instruction.b else {
                        return Err("branch offset".to_string());
                    };
                    frame.pc = ((frame.pc as isize) + (offset as isize)) as usize;
                }
            }
            Opcode::Closure => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("Closure dst".to_string());
                };
                let Operand::Prototype(child) = instruction.b else {
                    return Err("Closure child".to_string());
                };
                *frame.registers[dst.0 as usize].borrow_mut() = capture_closure(&frame, child);
            }
            Opcode::GetUpvalue => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("GetUpvalue dst".to_string());
                };
                let Operand::Upvalue(index) = instruction.b else {
                    return Err("GetUpvalue index".to_string());
                };
                *frame.registers[dst.0 as usize].borrow_mut() =
                    frame.upvalues[index as usize].borrow().clone();
            }
            Opcode::SetUpvalue => {
                let Operand::Upvalue(index) = instruction.a else {
                    return Err("SetUpvalue index".to_string());
                };
                let value = operand_value(&frame, &instruction.b)?;
                *frame.upvalues[index as usize].borrow_mut() = value;
            }
            Opcode::Concat => {
                binary_number_or_string(&mut frame, instruction, |lhs, rhs| {
                    Value::String(value_to_string(&lhs) + &value_to_string(&rhs))
                })?;
            }
            Opcode::Add => {
                binary_number(&mut frame, instruction, |lhs, rhs| lhs + rhs)?;
            }
            Opcode::Sub => {
                binary_number(&mut frame, instruction, |lhs, rhs| lhs - rhs)?;
            }
            Opcode::Mul => {
                binary_number(&mut frame, instruction, |lhs, rhs| lhs * rhs)?;
            }
            Opcode::Div => {
                binary_number(&mut frame, instruction, |lhs, rhs| lhs / rhs)?;
            }
            Opcode::Mod => {
                binary_number(&mut frame, instruction, |lhs, rhs| lhs % rhs)?;
            }
            Opcode::Pow => {
                binary_number(&mut frame, instruction, |lhs, rhs| lhs.powf(rhs))?;
            }
            Opcode::Eq | Opcode::Lt | Opcode::Le => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("compare dst".to_string());
                };
                let lhs = operand_value(&frame, &instruction.b)?;
                let rhs = operand_value(&frame, &instruction.c)?;
                let value = match instruction.opcode {
                    Opcode::Eq => lhs == rhs,
                    Opcode::Lt => lhs.expect_number()? < rhs.expect_number()?,
                    Opcode::Le => lhs.expect_number()? <= rhs.expect_number()?,
                    _ => unreachable!(),
                };
                *frame.registers[dst.0 as usize].borrow_mut() = Value::Boolean(value);
            }
            Opcode::Len => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("Len dst".to_string());
                };
                let len = match operand_value(&frame, &instruction.b)? {
                    Value::String(value) => value.chars().count() as f64,
                    Value::Table(table) => table_len(&table.borrow()) as f64,
                    other => return Err(format!("cannot take length of {other:?}")),
                };
                *frame.registers[dst.0 as usize].borrow_mut() = Value::Number(len);
            }
            Opcode::Not => {
                let Operand::Register(dst) = instruction.a else {
                    return Err("Not dst".to_string());
                };
                let value = operand_value(&frame, &instruction.b)?;
                let truthy = !matches!(value, Value::Nil | Value::Boolean(false));
                *frame.registers[dst.0 as usize].borrow_mut() = Value::Boolean(!truthy);
            }
            Opcode::CallSpread | Opcode::ReturnSpread => {
                return Err(format!("unsupported opcode {:?}", instruction.opcode));
            }
        }
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(value) => value.to_string(),
        Value::Number(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        Value::String(value) => value.clone(),
        Value::Table(_) => "table".to_string(),
        Value::NativeFunction(_) => "function".to_string(),
        Value::Closure(_) => "function".to_string(),
    }
}

fn binary_number(
    frame: &mut Frame,
    instruction: barred_luau::ir::Instruction,
    op: impl Fn(f64, f64) -> f64,
) -> VmResult<()> {
    let Operand::Register(dst) = instruction.a else {
        return Err("math dst".to_string());
    };
    let lhs = operand_value(frame, &instruction.b)?.expect_number()?;
    let rhs = operand_value(frame, &instruction.c)?.expect_number()?;
    *frame.registers[dst.0 as usize].borrow_mut() = Value::Number(op(lhs, rhs));
    Ok(())
}

fn binary_number_or_string(
    frame: &mut Frame,
    instruction: barred_luau::ir::Instruction,
    op: impl Fn(Value, Value) -> Value,
) -> VmResult<()> {
    let Operand::Register(dst) = instruction.a else {
        return Err("binary dst".to_string());
    };
    let lhs = operand_value(frame, &instruction.b)?;
    let rhs = operand_value(frame, &instruction.c)?;
    *frame.registers[dst.0 as usize].borrow_mut() = op(lhs, rhs);
    Ok(())
}

fn table_key(value: &Value) -> VmResult<TableKey> {
    match value {
        Value::String(value) => Ok(TableKey::String(value.clone())),
        Value::Number(value) if value.fract() == 0.0 => Ok(TableKey::Integer(*value as i64)),
        other => Err(format!("unsupported table key {other:?}")),
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Boolean(lhs), Value::Boolean(rhs)) => lhs == rhs,
            (Value::Number(lhs), Value::Number(rhs)) => lhs == rhs,
            (Value::String(lhs), Value::String(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}

fn run_source(source: &str) -> Vec<Value> {
    let program = Rc::new(compile_source(source));
    execute_proto(
        program.clone(),
        globals(),
        program.entry_prototype,
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
    )
    .expect("execute")
}

fn run_source_result(source: &str) -> VmResult<Vec<Value>> {
    let program = Rc::new(compile_source(source));
    execute_proto(
        program.clone(),
        globals(),
        program.entry_prototype,
        Vec::new(),
        BTreeMap::new(),
        Vec::new(),
    )
}

fn sample_output_prefix_source(tail: &str) -> String {
    let source = include_str!("../examples/sample_output.protected.luau");
    let (prefix, _) = source
        .rsplit_once("local function _zxhepgd()")
        .expect("sample output entrypoint");
    format!("{prefix}{tail}")
}

#[test]
fn vm_runtime_reconstructs_release_string_pool_entries() {
    let source = r#"
local function decode(t, k)
    local out = {}
    for i = 1, #t do
        out[i] = string.char(bit32.bxor(t[i], (k + i) % 256))
    end
    return table.concat(out)
end

local pool = {
    {30, {110,23,118,79,2,92,117,20,85,11,109,65,18,96,109,77,123,5,95,107,91,16,127,84,18,110,95,28,92,119,23,111,76,26,106,35,124,1}},
    {51, {14}},
}

local function get(i)
    local e = pool[i]
    return decode(e[2], e[1])
end

return get(1), get(2)
"#;

    let values = run_source(source);
    assert_eq!(
        values[0].expect_string().expect("alphabet"),
        "q7Wm!xP2r#Dk9L@cT5nYh$Jb%Vf&gK*QsZ+a?E"
    );
    assert_eq!(values[1].expect_string().expect("separator"), ":");
}

#[test]
fn vm_runtime_decodes_text_pairs_with_pooled_alphabet() {
    let alphabet = "q7Wm!xP2r#Dk9L@cT5nYh$Jb%Vf&gK*QsZ+a?E";
    let encoded = encode_blob(&[0, 255, 37], alphabet);
    let source = format!(
        r#"
local function decode(t, k)
    local out = {{}}
    for i = 1, #t do
        out[i] = string.char(bit32.bxor(t[i], (k + i) % 256))
    end
    return table.concat(out)
end

local pool = {{
    {{30, {{110,23,118,79,2,92,117,20,85,11,109,65,18,96,109,77,123,5,95,107,91,16,127,84,18,110,95,28,92,119,23,111,76,26,106,35,124,1}}}},
    {{51, {{14}}}},
}}

local function get(i)
    local e = pool[i]
    return decode(e[2], e[1])
end

local function maxPair(text, alphabet)
    local reverse = {{}}
    for i = 1, #alphabet do
        reverse[string.sub(alphabet, i, i)] = i - 1
    end

    local digits = {{}}
    for i = 1, #text do
        local ch = string.sub(text, i, i)
        if ch ~= get(2) then
            digits[#digits + 1] = reverse[ch]
        end
    end

    local radix = #alphabet
    local maxValue = 0
    for i = 1, #digits, 2 do
        local value = digits[i] * radix + digits[i + 1]
        if value > maxValue then
            maxValue = value
        end
    end
    return maxValue
end

return maxPair("{encoded}", get(1))
"#
    );

    let values = run_source(&source);
    assert_eq!(values[0].expect_number().expect("max pair"), 255.0);
}

#[test]
fn vm_runtime_handles_minified_runtime_helpers() {
    let source = r#"local _arjkjfh=function(t,k)local _gaekqid={}for i=1,#t do _gaekqid[i]=string.char(bit32.bxor(t[i],(k+i)%256))end return table.concat(_gaekqid)end local _baiuxuf={{30,{110,23,118,79,2,92,117,20,85,11,109,65,18,96,109,77,123,5,95,107,91,16,127,84,18,110,95,28,92,119,23,111,76,26,106,35,124,1}},{51,{14}}}local _oxhlgtl=function(i)local e=_baiuxuf[i]return _arjkjfh(e[2],e[1])end return _oxhlgtl(1),_oxhlgtl(2)"#;

    let values = run_source(source);
    assert_eq!(
        values[0].expect_string().expect("alphabet"),
        "q7Wm!xP2r#Dk9L@cT5nYh$Jb%Vf&gK*QsZ+a?E"
    );
    assert_eq!(values[1].expect_string().expect("separator"), ":");
}

#[test]
fn vm_runtime_preserves_strict_comparison_semantics() {
    let values = run_source(
        r#"
return 255 > 255, 255 >= 255, 256 > 255, 254 >= 255, 10 ~= 11
"#,
    );
    assert!(matches!(values[0], Value::Boolean(false)));
    assert!(matches!(values[1], Value::Boolean(true)));
    assert!(matches!(values[2], Value::Boolean(true)));
    assert!(matches!(values[3], Value::Boolean(false)));
    assert!(matches!(values[4], Value::Boolean(true)));
}

#[test]
#[ignore = "the lightweight Rust VM harness does not implement the full bootstrap runtime yet"]
fn vm_runtime_runs_sample_output_entrypoint() {
    let source = include_str!("../examples/sample_output.protected.luau");
    let values = run_source_result(source).expect("top-level");
    let entrypoint = values.into_iter().next().expect("top-level return value");
    let results = call_value(entrypoint, Vec::new()).expect("entrypoint");
    assert_eq!(results[0].expect_number().expect("result"), 19.0);
}

#[test]
fn vm_runtime_reconstructs_sample_output_alphabet() {
    let source = sample_output_prefix_source("return _oxhlgtl(1), #_oxhlgtl(1)\n");
    let values = run_source_result(&source).expect("alphabet");
    assert_eq!(
        values[0].expect_string().expect("alphabet"),
        "q7Wm!xP2r#Dk9L@cT5nYh$Jb%Vf&gK*QsZ+a?E"
    );
    assert_eq!(values[1].expect_number().expect("alphabet length"), 38.0);
}

#[test]
fn vm_runtime_decodes_sample_output_text_pairs() {
    let source = sample_output_prefix_source("return _msihvcc(_lepazse, _pblkvjn._exgdhxe)\n");
    let values = run_source_result(&source).expect("decode text pairs");
    let table = values[0].expect_table().expect("decoded bytes");
    assert!(table_len(&table.borrow()) > 0);
}
