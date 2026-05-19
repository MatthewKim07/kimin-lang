<div align="center">

# ⚡ Kimin

**An experimental systems and engineering language built from scratch in Rust**

*Physical units &nbsp;·&nbsp; State machines &nbsp;·&nbsp; Deterministic simulation — as first-class type system features*

![Tests](https://img.shields.io/badge/tests-879_passing-4caf50?style=flat-square)
![Rust](https://img.shields.io/badge/rust-2021_edition-orange?style=flat-square&logo=rust)
![Status](https://img.shields.io/badge/status-experimental-blue?style=flat-square)
![Milestone](https://img.shields.io/badge/milestone-9C-informational?style=flat-square)

</div>

---

Kimin is a programming language where **physical units, state machines, and simulation loops are part of the core type system** — not handled by libraries or naming conventions.

```
// Units are part of the type — enforced at compile time
let distance: meters = 100
let time: seconds = 10
let speed = distance / time   // type inferred: meters/seconds

// State machines are first-class language constructs
state Door {
  closed  opening  open
  transition closed -> opening
  transition opening -> open
}
let door: Door = Door.closed
transition door -> opening   // type checker validates the edge

// Deterministic simulation loop — no real-time delay
let mut position: meters = 0
let velocity: meters = 2
let duration: seconds = 3
let dt: seconds = 1

simulate duration step dt {
  position = position + velocity * dt
  print(position)   // 2 / 4 / 6
}
```

This is a from-scratch implementation: hand-written lexer, recursive-descent parser, static type checker, tree-walk interpreter, bytecode compiler, and stack-based VM — all in Rust, ~15k lines, **879 tests passing**.

---

## 🏗 Architecture

```
  source.kimin
       │
       ▼
  ┌─────────┐
  │  Lexer  │─── LexError (line, col)
  └────┬────┘
       │  token stream
       ▼
  ┌──────────┐
  │  Parser  │─── ParseError (line, col)
  └────┬─────┘
       │  AST
       ▼
  ┌─────────────┐
  │ TypeChecker │─── TypeError (line, col)
  └──────┬──────┘
         │  typed AST
    ┌────┴──────────────────────┐
    ▼                           ▼
Tree-walk Interpreter     Bytecode Compiler
  (primary path)               │
  kimin run                    ├── Disassembler  (kimin bytecode)
  kimin check                  │
  kimin repl                   └── Stack-based VM (kimin vm)
```

The tree-walk interpreter is the primary execution path and source of truth for language semantics. The bytecode backend is a complete parallel implementation covering the full feature set — both produce identical output.

---

## ✅ Features

### Core language

- Numbers (`42`, `3.14` — stored as `f64`), strings, booleans, nil
- Variables with optional type annotations: `let x: Number = 10`
- Arithmetic with correct precedence (`+`, `-`, `*`, `/`), comparisons, unary operators
- Blocks with lexical scope, `if`/`else`
- `print(expr)` statement, line comments (`// ...`), string concatenation

### Functions and closures

```
fn add(a: Number, b: Number) -> Number {
  return a + b
}
```

- Named functions with required typed parameters; optional return type annotation
- `return expr` / bare `return` (yields nil); implicit nil if no `return` reached
- Recursion, mutual recursion
- True lexical closures — functions capture their definition-site environment
- Nested functions, returned closures, free-variable capture

### Static type checker

- Runs as a separate pass: lex → parse → **type-check** → execute
- Three-pass scan: (1) register state machines, (2) register function signatures, (3) check everything
- Gradual typing via `Unknown` for unannotated return types — unannotated code stays valid
- All type errors include line and column
- Catches before execution: wrong argument types, arity mismatches, type annotation violations, undefined variables, `if` condition not `Bool`, return type mismatches, immutability violations

### Unit-aware types

Unit types are **static-only** — the runtime sees plain `f64`; zero overhead at execution.

| Expression | Result |
|---|---|
| `meters + meters` | `meters` |
| `meters + seconds` | **TypeError** |
| `Number * meters` | `meters` (scaling) |
| `meters / meters` | `Number` (dimensionless ratio) |
| `meters / seconds` | `meters/seconds` (compound, inferred) |
| `meters * meters` | `meters^2` |
| `(meters/seconds) * seconds` | `meters` (compound simplification) |
| `Number / seconds` | `1/seconds` (reciprocal) |

Supported units and aliases:

| Unit | Aliases |
|---|---|
| `meters` | `m` |
| `seconds` | `s` |
| `milliseconds` | `ms` |
| `minutes` | `min` |
| `hours` | `h` |
| `kilograms` | `kg` |
| `amperes` | `A`, `amps` |
| `kelvin` | `K` |
| `moles` | `mol` |
| `candela` | `cd` |
| `radians` | `rad` |
| `degrees` | `deg` |
| `volts` | `V` |
| `watts` | `W` |
| `joules` | `J` |
| `newtons` | `N` |

Compound unit types are inferred, not annotated. `let v: meters/seconds = ...` is a ParseError — the type comes from the expression.

### State machines

```
state TrafficLight {
  red  yellow  green
  transition red -> green
  transition green -> yellow
  transition yellow -> red
}

let light: TrafficLight = TrafficLight.red
transition light -> green    // ok — red -> green declared
transition light -> red      // TypeError: no green -> red transition declared
```

- State variant expressions: `Door.closed`, `TrafficLight.red`
- `transition` is the only mutation form for state-typed variables
- Static checking: validates that the transition edge is declared; rejects undeclared edges
- **Known-variant tracking**: the type checker records the statically known current variant and validates each subsequent transition against it
- Functions can accept and return state types

### Mutable variables and compound assignment

```
let mut counter: Number = 0
counter += 1    // 1
counter *= 3    // 3

let mut dist: meters = 0
let step: meters = 5
dist += step    // ok — same unit
// dist += 10   // TypeError — Number ≠ meters
```

- Immutable by default (`let`); `let mut` opts in to reassignment
- Full assignment: `x = expr`
- Compound assignment: `x += expr`, `x -= expr`, `x *= expr`, `x /= expr`
- Unit type rules apply to compound assignment — same rules as the corresponding binary operator
- State variables use `transition`, never assignment

### Simulation blocks

```
let mut position: meters = 0
let velocity: meters = 2
let duration: seconds = 3
let dt: seconds = 1

simulate duration step dt {
  position = position + velocity * dt
  print(position)   // 2 / 4 / 6
}
```

- `floor(duration / step)` deterministic iterations — no real-time waiting
- `time` variable injected into body (type matches duration unit)
- Duration and step must share the same time unit; plain `Number` → TypeError
- Time units: `seconds`/`s`, `milliseconds`/`ms`, `minutes`/`min`, `hours`/`h`
- Outer mutable variables and state transitions persist across iterations

### While loops and loop control

```
let mut x: Number = 0

while x < 10 {
    x += 1
    if x == 3 { continue }
    if x == 8 { break }
    print(x)
}
// 1 2 4 5 6 7
```

- Condition must be `Bool`; any other type → TypeError
- `break`: exits the nearest enclosing while loop
- `continue`: skips the rest of the body and re-evaluates the condition
- Both target the **nearest** enclosing while only; no labels
- Neither crosses function or simulate boundaries (loop context resets on entry)

### Bytecode backend

```sh
kimin bytecode examples/bytecode_demo.kimin   # print IR disassembly
kimin vm       examples/while.kimin           # execute via stack-based VM
```

- Flat bytecode IR with constant pool and jump patching
- Function chunks (`FunctionChunk`), simulate body chunks (`SimulateChunk`)
- Env-chain scope model (same as tree-walk interpreter) — closures via `Value::BytecodeFunction { name, env }`
- Dynamic/computed calls: callee expression compiled onto stack → `CALL arg_count`
- `while` loops lower to `JumpIfFalse`/`Jump`/`BeginScope`/`EndScope` — no new VM instructions
- `break`/`continue` lower to `EndScope × N + Jump` with `LoopContext` patch tracking
- Full parity with tree-walk output for all example files

---

## 🚀 Getting Started

Requires [Rust](https://rustup.rs/) (edition 2021).

```sh
git clone https://github.com/MatthewKim07/kimin-lang.git
cd kimin-lang
cargo build --release
```

Binary at `target/release/kimin`.

### CLI commands

| Command | Description |
|---|---|
| `kimin run <file>` | Run a `.kimin` file (tree-walk interpreter) |
| `kimin check <file>` | Type-check only — no execution |
| `kimin repl` | Interactive REPL with persistent type checker |
| `kimin bytecode <file>` | Print bytecode IR disassembly |
| `kimin vm <file>` | Execute via stack-based bytecode VM |

```sh
cargo run -- run examples/simulate_motion.kimin
cargo run -- run examples/states.kimin
cargo run -- check examples/typed_functions.kimin
cargo run -- bytecode examples/bytecode_demo.kimin
cargo run -- vm examples/vm_closure_capture.kimin
cargo run -- repl
```

---

## 📋 Examples

<details>
<summary><strong>Unit arithmetic and compound unit inference</strong></summary>

```
let distance: meters = 10
let time: seconds = 2
let speed = distance / time   // type: meters/seconds (inferred)
let back = speed * time       // type: meters (compound simplification)
print(speed)   // 5
print(back)    // 10
```

</details>

<details>
<summary><strong>State machines with transitions</strong></summary>

```
state Door {
  closed
  opening
  open

  transition closed -> opening
  transition opening -> open
}

let door: Door = Door.closed
print(door)              // Door.closed

transition door -> opening
print(door)              // Door.opening

transition door -> open
print(door)              // Door.open
```

</details>

<details>
<summary><strong>Simulation — motion with velocity</strong></summary>

```
let mut position: meters = 0

let dist_per_step: meters = 2
let unit_time: seconds = 1
let velocity = dist_per_step / unit_time   // meters/seconds

let duration: seconds = 3
let dt: seconds = 1

simulate duration step dt {
  position = position + velocity * dt
  print(position)   // 2 / 4 / 6
}
```

</details>

<details>
<summary><strong>Closures</strong></summary>

```
fn make_getter() {
  let x = 77
  fn get() { return x }
  return get
}

let getter = make_getter()
print(getter())   // 77 — captured env survives after make_getter returns
```

</details>

<details>
<summary><strong>Typed functions with unit parameters</strong></summary>

```
fn add_distance(a: meters, b: meters) -> meters {
  return a + b
}

fn scale(distance: meters, factor: Number) -> meters {
  return distance * factor
}

fn ratio(a: meters, b: meters) -> Number {
  return a / b
}

print(add_distance(10, 5))   // 15
print(scale(3, 4))           // 12
print(ratio(10, 2))          // 5
```

</details>

<details>
<summary><strong>Recursion</strong></summary>

```
fn fact(n: Number) -> Number {
  if n <= 1 { return 1 }
  return n * fact(n - 1)
}

print(fact(5))   // 120
```

</details>

---

## 🐛 Error Messages

Kimin catches most errors statically before execution:

```
TypeError at line 3, col 5:  cannot add meters and seconds
TypeError at line 2, col 5:  variable 'v' declared as meters but initializer has type meters/seconds
TypeError at line 7, col 1:  invalid transition for Door: closed -> closed
TypeError at line 6, col 18: unknown variant 'locked' for state machine 'Door'
TypeError at line 1, col 1:  function 'add' expected 2 arguments but got 1
TypeError at line 2, col 5:  cannot assign to immutable variable 'x'
TypeError at line 4, col 1:  'break' used outside of a while loop
TypeError:                   while condition must be Bool, got Number
ParseError at line 2, col 5: expected expression
LexError  at line 3, col 7:  unexpected character '@'
```

---

## ⚠️ Known Limitations

| Limitation | Notes |
|---|---|
| No anonymous functions | No lambda syntax |
| No `for` loops | Iteration via `while` and `simulate` only |
| No labeled `break`/`continue` | Targets nearest enclosing while only |
| No mutable function parameters | Parameters are always immutable |
| No compound unit annotations | `let v: meters/seconds = x` is a ParseError — inference only |
| No derived unit aliases | `kg*m/s²` does not reduce to `newtons`; no named derived units |
| No unit conversion | No SI prefixes; `meters` and `feet` are unrelated types |
| No time unit conversion | `minutes` and `seconds` are distinct non-interchangeable types |
| State transitions in functions | `transition` inside a function body modifies the local copy, not the caller's variable |
| `RuntimeError` has no source location | Runtime errors report message only — no line/col yet |
| Bytecode VM: recursive closure cycles | A function stored in its own captured env creates an `Rc` cycle → memory leak; harmless for run-and-exit programs |
| No multiline REPL input | Multi-line constructs (functions, while) must fit on one input line in the REPL |
| No package system | No module imports or namespacing |

---

## 🗺️ Roadmap

| Milestone | Focus | |
|---|---|---|
| 1 | Lexer, parser, AST, tree-walk interpreter, REPL | ✅ |
| 2A | Named functions, parameters, return, recursion | ✅ |
| 2B | Closures and lexical scoping | ✅ |
| 3 | Static type checker | ✅ |
| 4 | Unit-aware types | ✅ |
| 4B | Compound unit inference | ✅ |
| 5 | State machines | ✅ |
| 6A | `simulate` blocks | ✅ |
| 6B | Extended time units | ✅ |
| 7A | `let mut` and type-safe assignment | ✅ |
| 8A | Flat bytecode IR (`kimin bytecode`) | ✅ |
| 8B | Function chunks and named call lowering | ✅ |
| 8C | Stack-based bytecode VM (`kimin vm`) | ✅ |
| 8D | State machine execution in VM | ✅ |
| 8E | `simulate` block execution in VM | ✅ |
| 8F | Closure and free-variable capture in VM | ✅ |
| 8G | Dynamic/computed call execution in VM | ✅ |
| 9A | Compound assignment operators (`+=`, `-=`, `*=`, `/=`) | ✅ |
| 9B | While loops | ✅ |
| 9C | `break` and `continue` | ✅ |

---

## 🧪 Tests

```sh
cargo test
# 879 passed, 0 failed
```

Tests cover every layer: lexer, parser, type checker, interpreter, bytecode compiler, and VM — for all language features including edge cases and error conditions.

---

## 📁 Source

```
src/
  main.rs         CLI (clap) — run / check / repl / bytecode / vm
  token.rs        Token types and Span
  lexer.rs        Source → tokens
  ast.rs          Expression and statement AST nodes
  parser.rs       Recursive-descent parser + unit name registry
  typechecker.rs  Static type checker (TypeEnv, UnitDimension, State types, loop_depth)
  value.rs        Runtime values: Number, Text, Bool, Nil, Function, StateValue, BytecodeFunction
  env.rs          Lexical scope chain — Rc<RefCell<Env>>
  interpreter.rs  Tree-walk interpreter (ExecFlow: Normal / Return / Break / Continue)
  error.rs        Structured errors: Lex / Parse / Type / Runtime / Compile
  repl.rs         Interactive REPL with persistent type checker and interpreter
  bytecode.rs     Instruction enum, Chunk, FunctionChunk, SimulateChunk, BytecodeProgram
  compiler.rs     BytecodeCompiler — AST → flat bytecode (LoopContext for break/continue patching)
  disassemble.rs  Human-readable bytecode listing printer
  vm.rs           Stack-based VM — env-chain model, execute_chunk
  lib.rs          Module declarations
  tests.rs        879 unit tests
examples/
  hello.kimin                       arithmetic.kimin
  variables.kimin                   conditionals.kimin
  blocks.kimin                      errors.kimin
  functions.kimin                   return.kimin
  recursion.kimin                   function_errors.kimin
  lexical_scoping.kimin             closure.kimin
  types.kimin                       typed_functions.kimin
  type_errors.kimin                 units.kimin
  unit_functions.kimin              unit_errors.kimin
  compound_units.kimin              compound_unit_errors.kimin
  states.kimin                      state_errors.kimin
  state_functions.kimin             simulate.kimin
  simulate_state.kimin              simulate_errors.kimin
  simulate_time_units.kimin         simulate_time_unit_errors.kimin
  simulate_motion.kimin             mutable.kimin
  mutable_units.kimin               mutable_errors.kimin
  compound_assignment.kimin         compound_assignment_units.kimin
  simulate_compound_assignment.kimin  compound_assignment_errors.kimin
  while.kimin                       while_units.kimin
  while_function.kimin              while_state.kimin
  while_errors.kimin                break_continue.kimin
  break_continue_nested.kimin       break_continue_function.kimin
  break_continue_errors.kimin       bytecode_demo.kimin
  bytecode_functions.kimin          vm_demo.kimin
  vm_recursion.kimin                vm_simulate_state.kimin
  vm_closure_capture.kimin          vm_dynamic_calls.kimin
  vm_dynamic_adder.kimin
```

<details>
<summary><strong>Detailed feature notes by milestone</strong></summary>

### Milestone 1
- Integers and floats (`10`, `3.14`)
- Strings (`"hello"`)
- Booleans (`true`, `false`)
- Variables (`let x = 10`)
- Arithmetic with correct precedence (`+`, `-`, `*`, `/`)
- Parentheses for grouping
- Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`)
- Unary operators (`-x`, `!cond`)
- Blocks with lexical scope (`{ ... }`)
- `if` / `else`
- `print(expr)`
- Line comments (`// ...`)
- Interactive REPL
- Structured errors (lex / parse / runtime) with line and column info
- String concatenation via `+`

### Milestone 2A
- Named functions (`fn name(params) { ... }`)
- Function calls (`add(2, 3)`)
- Parameters and arguments (comma-separated)
- `return expr` and bare `return`
- Functions return `nil` if no `return` is reached
- Recursion
- Nested and chained calls (`square(add(2, 3))`)

### Milestone 2B
- Lexical (static) scoping — functions see variables from their definition site, not the call site
- True closures — functions capture their enclosing environment at declaration time
- Nested functions that capture outer locals
- Returned functions that keep their captured environment alive after the enclosing function returns
- Mutual recursion still works

### Milestone 3
- Static type checker pass (between parser and interpreter)
- Type annotations for variables: `let x: Number = 10` (optional; type inferred when omitted)
- Required type annotations for function parameters: `fn add(a: Number, b: Number)`
- Optional return type annotation: `fn add(a: Number, b: Number) -> Number`
- Built-in types: `Number`, `Text`, `Bool`, `Nil`
- Gradual typing: unannotated return types are `Unknown` — propagate without error
- `TypeError` errors with line and column info
- `kimin run` and `kimin check` both invoke the type checker
- REPL has a persistent type checker alongside the interpreter
- Caught statically: wrong argument types, return type mismatches, mismatched `let` annotations, `!` on non-Bool, `if` condition not Bool, cross-type equality, undefined variables, wrong arity, calling a non-function

### Milestone 4
- Unit-aware type annotations: `let distance: meters = 10`, `let time: seconds = 2`
- Unit types are **static-only** — the runtime sees plain numbers; no overhead at execution time
- 13 built-in units with short aliases: `m`/`meters`, `s`/`seconds`, `kg`/`kilograms`, `A`/`amps`/`amperes`, `K`/`kelvin`, `mol`/`moles`, `cd`/`candela`, `rad`/`radians`, `deg`/`degrees`, `V`/`volts`, `W`/`watts`, `J`/`joules`, `N`/`newtons`
- Unit arithmetic rules enforced by the type checker
- Number literals promote to unit type at assignment (`let d: meters = 10` is valid)
- Unit-typed function parameters and return types: `fn add_dist(a: meters, b: meters) -> meters`

### Milestone 4B
- **Compound unit inference**: the type checker infers compound unit types from `*` and `/`
  - `meters / seconds → meters/seconds` (inferred, not annotated)
  - `meters * meters → meters^2`
  - `(meters/seconds) * seconds → meters` (compound simplification)
  - `Number / seconds → 1/seconds` (reciprocal)
- No new source annotation syntax — compound types are inferred only
- No runtime changes

### Milestone 5
- **State machine declarations**: `state Name { variant1  variant2  transition v1 -> v2 }`
- **State variable binding**: `let door: Door = Door.closed`
- **Controlled transition statements**: `transition door -> opening`
- **Static transition checking**: validates transitions against declared rules
- **Known-variant tracking**: type checker updates after each transition
- **State types in functions**: `fn foo(d: Door) -> Door { ... }`
- `transition` is a controlled mutation statement — NOT general variable assignment

### Milestone 6A
- **`simulate` blocks**: `simulate <duration> step <step> { statements }`
  - Deterministic loop: `floor(duration / step)` iterations, no real-time waiting
  - `time` variable injected into the body scope, type matches duration unit
  - Duration and step must be a time unit (or `Unknown` for gradual typing)
  - State transitions inside the body persist across iterations

### Milestone 6B
- **Extended time units for `simulate`**: `milliseconds`/`ms`, `minutes`/`min`, `hours`/`h` now accepted in addition to `seconds`/`s`

### Milestone 7A
- **`let mut` and assignment**: disciplined, type-safe variable reassignment
  - Variables are immutable by default; only `let mut` bindings may be reassigned
  - `let mut x: Number = 0` declares a mutable variable; `x = x + 1` reassigns it
  - State variables must use `transition` — direct assignment is a `TypeError`
  - Assignment is a statement, not an expression

### Milestone 8A
- **Bytecode IR emission**: `kimin bytecode <file>` compiles source and prints a flat bytecode listing
  - `Instruction` enum: literals, globals/locals, arithmetic, comparisons, print, control flow, scoping, return
  - Jump patching: `JumpIfFalse` and `Jump` targets filled in after branch body is emitted
  - `CompileError` type added; `KiminError::Compile` variant added

### Milestone 8B
- **Function chunks**: `BytecodeProgram { main: Chunk, functions: Vec<FunctionChunk> }`
  - Function declarations lower to `LOAD_FUNCTION name` + `DEFINE_GLOBAL name` in main chunk
  - Bodies without `return` receive implicit `NIL + RETURN`

### Milestone 8C
- **Bytecode VM**: `kimin vm <file>` executes `.kimin` files through the bytecode compiler and a stack-based VM
  - Division by zero, undefined variables, and wrong-arity calls produce clean `RuntimeError`
  - `kimin run` unchanged — tree-walk interpreter remains source of truth

### Milestones 8D–8G
- **8D**: State machine execution in VM (`DefineState`, `LoadState`, `Transition` instructions)
- **8E**: Simulate block execution in VM (`SimulateChunk`, `Instruction::Simulate { body_idx }`)
- **8F**: Closure and free-variable capture in VM — `Value::BytecodeFunction { name, env: EnvRef }` carries definition-site env; env-chain model replaces flat HashMap
- **8G**: Dynamic/computed call execution — `Call { arg_count }` pops callee from stack; all callee shapes share one dispatch path

### Milestone 9A
- **Compound assignment operators** (`+=`, `-=`, `*=`, `/=`): unit-safe in-place mutation for `let mut` variables
  - Bytecode compiler desugars to `Load/op/Store` — no new VM instructions

### Milestone 9B
- **While loops** (`while <condition> { <body> }`)
  - Condition must have type `Bool`
  - Body has a fresh lexical scope per iteration
  - Bytecode: `JumpIfFalse`/`Jump`/`BeginScope`/`EndScope` — no new VM instructions

### Milestone 9C
- **`break`**: exits the nearest enclosing while loop immediately
- **`continue`**: skips the rest of the current while-body iteration and re-evaluates the condition
  - Both are valid only inside a `while` loop; using either outside → `TypeError`
  - `break`/`continue` do not cross function or simulate boundaries
  - Bytecode: `EndScope × N + Jump` with `LoopContext` patch tracking — no new VM instructions

</details>
