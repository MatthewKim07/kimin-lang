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

    // Functions
    /// Push a reference to a named function from the function table.
    LoadFunction(String),
    /// Call a named function with the given number of arguments already on the stack.
    Call {
        name: String,
        arg_count: usize,
    },

    // Function control
    Return,
    Halt,

    // State machines
    /// Register a state machine definition in the VM state registry. No stack effect.
    DefineState {
        name: String,
        variants: Vec<String>,
        transitions: Vec<(String, String)>,
    },
    /// Push Value::StateValue { state_name, variant_name } onto the stack.
    LoadState {
        state_name: String,
        variant_name: String,
    },
    /// Controlled state transition: update an existing variable in-place.
    Transition {
        variable: String,
        target: String,
    },

    /// Placeholder for language features not yet lowered (simulate).
    Unsupported(String),
}

/// A sequence of instructions paired with a constant pool.
#[derive(Debug, Default, Clone)]
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

/// Bytecode for a single named function.
#[derive(Debug, Clone)]
pub struct FunctionChunk {
    pub name: String,
    pub params: Vec<String>,
    pub arity: usize,
    pub chunk: Chunk,
}

/// The compiled output for a whole Kimin program.
/// `main` is the top-level chunk; `functions` holds each named function's bytecode
/// in source order.
#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    pub main: Chunk,
    pub functions: Vec<FunctionChunk>,
}

impl BytecodeProgram {
    pub fn new(main: Chunk, functions: Vec<FunctionChunk>) -> Self {
        BytecodeProgram { main, functions }
    }
}
