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

    /// Build a fixed-size array from the top N stack values (leftmost element pushed first).
    Array {
        count: usize,
    },
    /// Build a map from the top N key-value pairs on the stack.
    /// Entries compiled as key1, value1, key2, value2, ... left-to-right.
    /// Duplicate keys: later entry in source order wins.
    Map {
        count: usize,
    },
    /// Index into an array. Stack: [..., array, index] → element.
    Index,
    /// Return the length of an array. Stack: [..., array] → Number.
    Len,
    /// Slice an array. Stack: [..., array, start, end] → new_array.
    /// End-exclusive; returns an independent copy of the sub-array.
    Slice,
    /// Append a value to a mutable array variable.
    /// Stack before: [..., new_value] — pops value, pushes Nil.
    ArrayPush(String),
    /// Remove and return the last element of a mutable array variable.
    /// No stack input needed — pushes popped element (or errors if empty).
    ArrayPop(String),

    /// Assign to an array element by index.
    /// Stack before: [..., index_value, new_value]
    /// Pops both; looks up `name` in the env chain; validates and clones the array;
    /// updates the element; assigns the updated array back to the existing binding.
    SetIndex(String),
    /// Compound-assign to an array element: `arr[i] op= rhs`.
    /// Stack before: [..., index_value, rhs_value]
    /// Pops rhs, pops index; reads old element at index; applies op(old, rhs);
    /// clones Vec, replaces element, assigns updated array back.
    IndexCompoundAssign {
        name: String,
        op: crate::ast::CompoundAssignOp,
    },

    /// String utility builtins: stack [..., text, pattern] → Bool.
    Contains,
    StartsWith,
    EndsWith,

    /// String transformation builtins: stack [..., text] → Text.
    ToUpper,
    ToLower,
    Trim,

    /// Convert any value to its display string. Stack: [..., value] → Text.
    /// Uses the same deterministic formatting as `print`.
    ToString,

    /// Split a string by a delimiter. Stack: [..., text, delimiter] → Array<Text>.
    /// Empty delimiter splits into individual characters.
    Split,

    /// Join an Array<Text> with a delimiter. Stack: [..., parts, delimiter] → Text.
    /// Empty array → ""; empty delimiter → concatenation.
    Join,

    /// Check if a key exists in a map. Stack: [..., map, key] → Bool.
    /// Missing key returns false (not a RuntimeError).
    HasKey,

    /// Return all keys of a map as Array<Text>. Stack: [..., map] → Array<Text>.
    /// Keys are in deterministic sorted (BTreeMap lexicographic) order.
    Keys,

    /// Return all values of a map as Array<V>. Stack: [..., map] → Array<V>.
    /// Values are in deterministic sorted-key (BTreeMap lexicographic) order,
    /// matching the order of Keys.
    Values,

    /// Remove a key from a named mutable map variable and push the removed value.
    /// Stack: [..., key] → removed_value.
    /// The map variable `name` is mutated in the env chain.
    /// Missing key → RuntimeError.
    RemoveKey(String),

    /// Construct a struct value from field values on the stack.
    /// Field values are pushed in source order (left-to-right). VM pops them in LIFO
    /// order (reverses to restore source order) and maps each to its declared field name.
    /// Stack: [..., val1, val2, ... valN] → Struct { name, fields: { f1:v1, f2:v2, ... } }
    StructLiteral {
        name: String,
        fields: Vec<String>,
    },

    /// Read a named field from a struct value on the stack.
    /// Stack: [..., struct_value] → field_value.
    FieldAccess(String),

    /// Assign a value through a path rooted at a named variable.
    ///
    /// Stack before: [..., index0_val, ..., indexN_val, new_value]
    /// where index_k is the k-th Index step's value compiled left-to-right.
    ///
    /// Pops new_value (top), then pops N index values (reverses to left-to-right order),
    /// loads root variable from env, walks path (Field steps use embedded name; Index steps
    /// consume index values), updates the final location, assigns updated root back.
    SetPath {
        root: String,
        steps: Vec<crate::ast::PathStep>,
    },

    /// Compound-assign through a path: `target op= rhs`.
    ///
    /// Stack before: [..., index0_val, ..., indexN_val, rhs_value]
    /// Pops rhs, pops N index values (reverses to order), reads old value at path,
    /// applies op(old, rhs), writes new value back through path.
    PathCompoundAssign {
        root: String,
        steps: Vec<crate::ast::PathStep>,
        op: crate::ast::CompoundAssignOp,
    },

    /// Call a method on a struct receiver.
    ///
    /// Stack before: [..., receiver, arg1, ..., argN]
    /// Pops N explicit args (reverses to source order) then pops receiver.
    /// Dispatches by receiver's struct name + method name.
    /// Pushes return value; matching CALL semantics.
    CallMethod {
        method: String,
        arg_count: usize,
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

/// Bytecode for a single struct method.
#[derive(Debug, Clone)]
pub struct MethodChunk {
    pub struct_name: String,
    pub method_name: String,
    /// Parameter names in source order; first is always `"self"`.
    pub params: Vec<String>,
    pub arity: usize,
    pub chunk: Chunk,
}

/// The compiled output for a whole Kimin program.
/// `main` is the top-level chunk; `functions` holds each named function's bytecode
/// in source order; `simulate_bodies` holds each simulate body chunk in source order;
/// `methods` holds each struct method chunk indexed by (struct_name, method_name).
#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    pub main: Chunk,
    pub functions: Vec<FunctionChunk>,
    pub simulate_bodies: Vec<SimulateChunk>,
    pub methods: Vec<MethodChunk>,
}

impl BytecodeProgram {
    pub fn new(
        main: Chunk,
        functions: Vec<FunctionChunk>,
        simulate_bodies: Vec<SimulateChunk>,
        methods: Vec<MethodChunk>,
    ) -> Self {
        BytecodeProgram {
            main,
            functions,
            simulate_bodies,
            methods,
        }
    }
}
