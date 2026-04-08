use std::collections::{HashMap, HashSet};

use crate::compiler::label_resolver::{Label, LabelResolver};
use crate::error::CompileError;
use crate::ir::{
    ConstantPoolExt, ConstantValue, FunctionProto, FunctionProtoExt, Instruction, InstructionExt,
    Opcode, Operand, PrototypeId, RegisterAllocator, RegisterId,
};

#[derive(Clone, Debug)]
pub struct ParentBindings {
    names: HashSet<String>,
    parent: Option<Box<ParentBindings>>,
}

impl ParentBindings {
    pub fn new() -> Self {
        Self {
            names: HashSet::new(),
            parent: None,
        }
    }

    pub fn child_of(parent: &ParentBindings, names: HashSet<String>) -> Self {
        Self {
            names,
            parent: Some(Box::new(parent.clone())),
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.contains(name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariableResolution {
    Local(RegisterId),
    Upvalue(u16),
    Global,
}

#[derive(Clone, Debug)]
pub struct FunctionCompileContext {
    pub proto: FunctionProto,
    pub allocator: RegisterAllocator,
    pub scopes: Vec<HashMap<String, RegisterId>>,
    pub upvalue_map: HashMap<String, u16>,
    pub label_resolver: LabelResolver,
    pub parent_bindings: ParentBindings,
}

impl FunctionCompileContext {
    pub fn new(name: Option<String>, params: &[String], parent_bindings: ParentBindings) -> Self {
        let mut ctx = Self {
            proto: FunctionProto {
                name,
                parameters: params.to_vec(),
                ..FunctionProto::default()
            },
            allocator: RegisterAllocator::default(),
            scopes: vec![HashMap::new()],
            upvalue_map: HashMap::new(),
            label_resolver: LabelResolver::default(),
            parent_bindings,
        };
        for parameter in params {
            let register = ctx.declare_local(parameter.clone());
            ctx.proto
                .set_local_name(register.0, Some(parameter.clone()));
        }
        ctx
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn declare_local(&mut self, name: String) -> RegisterId {
        let register = self.allocator.alloc();
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.clone(), register);
        }
        self.proto.set_local_name(register.0, Some(name));
        register
    }

    pub fn alloc_temp(&mut self) -> RegisterId {
        self.allocator.alloc()
    }

    pub fn alloc_block(&mut self, count: u16) -> RegisterId {
        self.allocator.alloc_block(count)
    }

    pub fn resolve_variable(&mut self, name: &str) -> VariableResolution {
        for scope in self.scopes.iter().rev() {
            if let Some(register) = scope.get(name) {
                return VariableResolution::Local(*register);
            }
        }

        if let Some(index) = self.upvalue_map.get(name) {
            return VariableResolution::Upvalue(*index);
        }

        if self.parent_bindings.contains(name) {
            let index = self.upvalue_map.len() as u16;
            self.upvalue_map.insert(name.to_string(), index);
            self.proto.upvalues.push(name.to_string());
            return VariableResolution::Upvalue(index);
        }

        VariableResolution::Global
    }

    pub fn export_bindings(&self) -> ParentBindings {
        let mut names = HashSet::new();
        for scope in &self.scopes {
            names.extend(scope.keys().cloned());
        }
        names.extend(self.upvalue_map.keys().cloned());
        ParentBindings::child_of(&self.parent_bindings, names)
    }

    pub fn emit(&mut self, instruction: Instruction) -> usize {
        self.proto.push_instruction(instruction)
    }

    pub fn emit_jump(&mut self, opcode: Opcode, condition: Option<RegisterId>, label: Label) {
        let instruction = match condition {
            Some(register) => Instruction::new(
                opcode,
                Operand::Register(register),
                Operand::Immediate(0),
                Operand::None,
            ),
            None => Instruction::new(opcode, Operand::None, Operand::Immediate(0), Operand::None),
        };
        let index = self.emit(instruction);
        self.label_resolver.patch_jump_b(index, label);
    }

    pub fn new_label(&mut self) -> Label {
        self.label_resolver.new_label()
    }

    pub fn mark_label(&mut self, label: Label) {
        let position = self.proto.instructions.len();
        self.label_resolver.mark(label, position);
    }

    pub fn intern_constant(&mut self, value: ConstantValue) -> crate::ir::ConstantId {
        self.proto.constants.intern(value)
    }

    pub fn finalize(mut self) -> Result<FunctionProto, CompileError> {
        self.proto.max_registers = self.allocator.high_water_mark();
        self.label_resolver.resolve(&mut self.proto)?;
        Ok(self.proto)
    }

    pub fn emit_load_nil(&mut self, dst: RegisterId) {
        self.emit(Instruction::new(
            Opcode::LoadNil,
            Operand::Register(dst),
            Operand::None,
            Operand::None,
        ));
    }

    pub fn emit_move(&mut self, dst: RegisterId, src: RegisterId) {
        self.emit(Instruction::new(
            Opcode::Move,
            Operand::Register(dst),
            Operand::Register(src),
            Operand::None,
        ));
    }

    pub fn emit_closure(&mut self, dst: RegisterId, proto: PrototypeId) {
        self.emit(Instruction::new(
            Opcode::Closure,
            Operand::Register(dst),
            Operand::Prototype(proto),
            Operand::None,
        ));
    }
}
