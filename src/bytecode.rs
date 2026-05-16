/// A compile-time constant stored in the constant pool.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Number(f64),
    Text(String),
    Bool(bool),
    Nil,
}

/// A single bytecode instruction for the Kimin IR.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // Literals
    Constant(usize),
    Nil,
    True,
    False,

    // Variable access / mutation
    LoadGlobal(String),
    DefineGlobal(String),
    StoreGlobal(String),
    LoadLocal(String),
    DefineLocal(String),
    StoreLocal(String),

    // Arithmetic / logic
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
    Not,

    // Comparison
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,

    // I/O
    Print,
    Pop,

    // Control flow
    JumpIfFalse(usize),
    Jump(usize),

    // Scoping
    BeginScope,
    EndScope,

    // Function control
    Return,
    Halt,

    /// Placeholder for language features not yet lowered (functions, states, simulate).
    Unsupported(String),
}

/// A sequence of instructions paired with a constant pool.
#[derive(Debug, Default)]
pub struct Chunk {
    pub instructions: Vec<Instruction>,
    pub constants: Vec<Constant>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk::default()
    }

    /// Adds a constant to the pool. Returns its index.
    pub fn add_constant(&mut self, c: Constant) -> usize {
        self.constants.push(c);
        self.constants.len() - 1
    }

    /// Appends an instruction. Returns its index (used for jump patching).
    pub fn emit(&mut self, instr: Instruction) -> usize {
        self.instructions.push(instr);
        self.instructions.len() - 1
    }
}

/// The compiled output for a whole Kimin program (single flat chunk for M8A).
#[derive(Debug)]
pub struct BytecodeProgram {
    pub chunk: Chunk,
}

impl BytecodeProgram {
    pub fn new(chunk: Chunk) -> Self {
        BytecodeProgram { chunk }
    }
}
