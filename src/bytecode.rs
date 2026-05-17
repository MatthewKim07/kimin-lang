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
    /// Stack-based call. Before this instruction:
    ///   stack: [..., callee_value, arg1, ..., argN]
    /// Pops N args, pops callee, invokes callee(args). Pushes return value.
    Call {
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

    /// Execute a simulate loop. Duration and step are already on the stack (duration first, then step).
    /// The body is stored in `BytecodeProgram.simulate_bodies[body_idx]`.
    Simulate {
        body_idx: usize,
    },

    /// Placeholder for language features not yet lowered (dynamic calls, closures).
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

/// Bytecode for a single simulate body.
/// `name` is a stable identifier like `"simulate#0"` used by the disassembler.
#[derive(Debug, Clone)]
pub struct SimulateChunk {
    pub name: String,
    pub chunk: Chunk,
}

/// The compiled output for a whole Kimin program.
/// `main` is the top-level chunk; `functions` holds each named function's bytecode
/// in source order; `simulate_bodies` holds each simulate body chunk in source order.
#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    pub main: Chunk,
    pub functions: Vec<FunctionChunk>,
    pub simulate_bodies: Vec<SimulateChunk>,
}

impl BytecodeProgram {
    pub fn new(
        main: Chunk,
        functions: Vec<FunctionChunk>,
        simulate_bodies: Vec<SimulateChunk>,
    ) -> Self {
        BytecodeProgram {
            main,
            functions,
            simulate_bodies,
        }
    }
}
