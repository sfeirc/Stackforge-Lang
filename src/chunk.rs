use crate::value::Value;

/// A single bytecode instruction. Represented as a Rust enum (rather than
/// raw bytes with a separate opcode table) so the compiler's unit tests can
/// assert on the exact instruction sequence emitted for a given program, and
/// so the VM's dispatch loop is a plain `match` -- see "Honest scope" in the
/// README for why this is a deliberate trade (inspectability over
/// bit-packing) and not a shortcut around "real" bytecode.
///
/// Jump targets are absolute indices into the owning `Chunk`'s `code` vector,
/// patched after the jump destination is known (classic backpatching).
#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    PushConst(usize),
    PushNil,
    PushTrue,
    PushFalse,
    Pop,

    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Not,

    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    GetLocal(usize),
    SetLocal(usize),

    Jump(usize),
    JumpIfFalse(usize),
    /// Peeks (does not pop) the top of stack; jumps if falsy. Used to
    /// compile short-circuiting `&&`.
    JumpIfFalsePeek(usize),
    /// Peeks (does not pop) the top of stack; jumps if truthy. Used to
    /// compile short-circuiting `||`.
    JumpIfTruePeek(usize),

    BuildArray(usize),
    BuildMap(usize),
    IndexGet,
    IndexSet,

    /// Call user function at `functions[index]` with `argc` arguments
    /// already pushed on the stack.
    Call(usize, u8),
    /// Call a native/builtin function (`len`, `push`, ...) by id.
    CallBuiltin(usize, u8),
    Print(u8),
    Return,
}

#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub lines: Vec<u32>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn emit(&mut self, op: OpCode, line: u32) -> usize {
        self.code.push(op);
        self.lines.push(line);
        self.code.len() - 1
    }

    pub fn add_constant(&mut self, v: Value) -> usize {
        self.constants.push(v);
        self.constants.len() - 1
    }

    pub fn patch_jump(&mut self, at: usize, target: usize) {
        self.code[at] = match &self.code[at] {
            OpCode::Jump(_) => OpCode::Jump(target),
            OpCode::JumpIfFalse(_) => OpCode::JumpIfFalse(target),
            OpCode::JumpIfFalsePeek(_) => OpCode::JumpIfFalsePeek(target),
            OpCode::JumpIfTruePeek(_) => OpCode::JumpIfTruePeek(target),
            other => panic!("patch_jump called on non-jump opcode {:?}", other),
        };
    }

    pub fn here(&self) -> usize {
        self.code.len()
    }
}

/// A compiled function: its parameter count and its own instruction chunk.
/// Index 0 in the VM's function table is always the compiled top-level
/// script ("main"), so `Call` never needs a special case for it.
#[derive(Debug)]
pub struct FunctionObj {
    pub name: String,
    pub arity: usize,
    pub chunk: Chunk,
}
