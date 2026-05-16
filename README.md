# Kimin

An experimental programming language designed by Matthew Kim. Kimin is being built as a modern systems/engineering language where **units, time, state, and constraints** will eventually become first-class language features.

This repository contains **Milestone 8B**: function and call lowering in the bytecode IR. Built on top of the Milestone 8A flat bytecode layer.

---

## Features

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
- Unit arithmetic rules enforced by the type checker:
  - `meters + meters → meters` (same unit, ok)
  - `meters + seconds → TypeError` (different units)
  - `Number * meters → meters` (scalar scaling)
  - `meters / meters → Number` (same-unit division gives dimensionless result)
- Number literals promote to unit type at assignment (`let d: meters = 10` is valid)
- Unit-typed function parameters and return types: `fn add_dist(a: meters, b: meters) -> meters`
- Number literals promote in function call arguments (`add_dist(10, 5)` is valid)

### Milestone 4B
- **Compound unit inference**: the type checker infers compound unit types from `*` and `/`
  - `meters / seconds → meters/seconds` (inferred, not annotated)
  - `meters * seconds → meters*seconds`
  - `meters * meters → meters^2`
  - `(meters/seconds) * seconds → meters` (compound simplification)
  - `Number / seconds → 1/seconds` (reciprocal)
  - `meters/seconds / meters/seconds → Number` (same compound unit divides to dimensionless)
- Compound unit types display as `meters/seconds`, `meters^2`, `1/seconds`, `kilograms*meters/seconds^2`
- No new source annotation syntax — compound types are inferred only; annotations remain single base units
- No runtime changes

### Milestone 5
- **State machine declarations**: `state Name { variant1  variant2  transition v1 -> v2 }`
- **State variable binding**: `let door: Door = Door.closed`
- **Controlled transition statements**: `transition door -> opening`
- **Static transition checking**: the type checker validates transitions against declared rules
  - `transition door -> opening` where `closed -> opening` is declared: ok
  - `transition door -> open` where `closed -> open` is NOT declared: `TypeError: invalid transition for Door: closed -> open`
  - `transition door -> locked` where `locked` is not a variant: `TypeError: unknown variant 'locked' for state machine 'Door'`
  - `transition x -> open` where `x` is a `Number`: `TypeError: 'x' has type Number, not a state machine`
- **Known-variant tracking**: the type checker tracks statically known current variant and updates it after each transition
- **State types in functions**: `fn foo(d: Door) -> Door { ... }` — functions accept and return state types
- **Runtime state values** print as `Door.closed`, `Door.opening`, etc.
- `transition` is a controlled mutation statement — NOT general variable assignment

### Milestone 6A
- **`simulate` blocks**: `simulate <duration> step <step> { statements }`
  - Deterministic loop: `floor(duration / step)` iterations, no real-time waiting
  - `time` variable injected into the body scope, type matches duration unit
  - Duration and step must be a time unit (or `Unknown` for gradual typing)
  - State transitions inside the body persist across iterations (outer variable mutated via `assign_existing`)
- **Static checks**:
  - Duration must be a time unit — plain `Number` is a `TypeError`
  - Step unit must match duration unit exactly
  - `time` is undefined outside the simulate block
- **Runtime checks**:
  - `step <= 0` → `RuntimeError`
  - `duration < 0` → `RuntimeError`

### Milestone 6B
- **Extended time units for `simulate`**: `milliseconds`/`ms`, `minutes`/`min`, `hours`/`h` now accepted in addition to `seconds`/`s`
  - Duration and step must share the exact same canonical time unit — no conversion between units
  - `ms` and `milliseconds` are identical types; same for `min`/`minutes` and `h`/`hours`

### Milestone 7A
- **`let mut` and assignment**: disciplined, type-safe variable reassignment
  - Variables are immutable by default; only `let mut` bindings may be reassigned
  - `let mut x: Number = 0` declares a mutable variable
  - `x = x + 1` reassigns it — type must be preserved; Number promotes to unit at assignment site
  - State variables must use `transition` — `door = Door.open` is a `TypeError`
  - Assignment is a statement, not an expression; no `+=` or compound operators
- **`simulate` + mutation** — mutable outer variables update across iterations:
  ```
  let mut position: meters = 0
  let dist_per_step: meters = 2
  let unit_time: seconds = 1
  let velocity = dist_per_step / unit_time
  let duration: seconds = 3
  let dt: seconds = 1
  simulate duration step dt {
    position = position + velocity * dt
    print(position)
  }
  // 2 / 4 / 6
  ```

### Milestone 8A
- **Bytecode IR emission**: `kimin bytecode <file>` compiles a source file and prints a human-readable flat bytecode listing
  - New modules: `bytecode.rs` (types), `compiler.rs` (lowering), `disassemble.rs` (printer)
  - `Instruction` enum covers literals, globals/locals, arithmetic, comparisons, print, control flow, scoping, and return
  - Constant pool: numbers, strings, booleans, nil
  - Jump patching: `JumpIfFalse` and `Jump` targets are filled in after the branch body is emitted
  - Local scope: variables inside `{ ... }` blocks emit `DefineLocal`/`LoadLocal`/`StoreLocal`; top-level emit `DefineGlobal`/`LoadGlobal`/`StoreGlobal`
  - `CompileError` type added; `KiminError::Compile` variant added
- **`kimin bytecode` CLI command**: emits the disassembly listing to stdout; lex/parse/typecheck errors still reported normally

### Milestone 8B
- **Function chunks**: `BytecodeProgram` now contains `main: Chunk` and `functions: Vec<FunctionChunk>`
  - Each `FunctionChunk` stores `name`, `params`, `arity`, and its own `Chunk`
  - Function declarations lower to `LOAD_FUNCTION name` + `DEFINE_GLOBAL name` in the main chunk and a separate function chunk
  - Function parameters are pre-seeded as locals; body `let` bindings also emit `DefineLocal`/`LoadLocal`
  - Bodies without an explicit `return` receive an implicit `NIL` + `RETURN` at the end
- **Named call lowering**: `Expr::Call` with a simple variable callee lowers to argument expressions followed by `CALL name arg_count`
  - Recursive calls (e.g., `fact(n - 1)`) correctly emit `CALL fact 1` inside the function chunk
  - Nested calls (e.g., `square(add(2, 3))`) emit inner call before outer call
  - Dynamic (computed) callees emit `UNSUPPORTED(dynamic call)` — not yet lowered
- **Disassembler**: prints each function chunk after main with header `=== function name/arity ===` and `params:` line
- State machines, `transition`, and `simulate` still emit `Unsupported(...)` markers
- Tree-walk interpreter is unchanged and remains the execution source of truth; bytecode IR is not yet executed

---

## Install

Requires [Rust](https://rustup.rs/) (edition 2021).

```sh
git clone https://github.com/MatthewKim07/kimin-lang.git
cd kimin-lang
cargo build --release
```

The binary is at `target/release/kimin`.

---

## Usage

### Run a file

```sh
cargo run -- run examples/functions.kimin
```

### Check syntax and types (no execution)

```sh
cargo run -- check examples/functions.kimin
```

### Start the REPL

```sh
cargo run -- repl
```

### Print bytecode IR

```sh
cargo run -- bytecode examples/bytecode_demo.kimin
```

---

## Examples

### functions.kimin

```kimin
fn add(a: Number, b: Number) -> Number {
  return a + b
}

fn square(x: Number) -> Number {
  return x * x
}

print(add(2, 3))
print(square(5))
print(square(add(2, 3)))
```

```
5
25
25
```

### simulate.kimin — time simulation block

```kimin
let duration: seconds = 3
let dt: seconds = 1

simulate duration step dt {
  print(time)
}
```

```
0
1
2
```

### states.kimin — state machines

```kimin
state Door {
  closed
  opening
  open

  transition closed -> opening
  transition opening -> open
}

let door: Door = Door.closed
print(door)

transition door -> opening
print(door)

transition door -> open
print(door)
```

```
Door.closed
Door.opening
Door.open
```

### compound_units.kimin — compound unit inference

```kimin
let distance: meters = 10
let time: seconds = 2
let speed = distance / time   // type: meters/seconds
let back = speed * time       // type: meters (simplification)
print(speed)   // 5
print(back)    // 10
```

```
5
10
```

### units.kimin — unit-aware types

```kimin
let distance: meters = 10
let extra: meters = 5
let total = distance + extra

let time: seconds = 2
let more_time: s = 3
let total_time = time + more_time

print(total)
print(total_time)
```

```
15
5
```

### unit_functions.kimin — functions with unit types

```kimin
fn add_distance(a: meters, b: meters) -> meters {
  return a + b
}

fn scale_distance(distance: meters, factor: Number) -> meters {
  return distance * factor
}

fn ratio(a: meters, b: meters) -> Number {
  return a / b
}

print(add_distance(10, 5))
print(scale_distance(3, 4))
print(ratio(10, 2))
```

```
15
12
5
```

### types.kimin — type annotations

```kimin
let count: Number = 42
let name: Text = "Kimin"
let active: Bool = true

print(count)
print(name)
print(active)
```

```
42
Kimin
true
```

### recursion.kimin

```kimin
fn fact(n: Number) -> Number {
  if n <= 1 {
    return 1
  }
  return n * fact(n - 1)
}

print(fact(5))
```

```
120
```

### conditionals.kimin

```kimin
let score = 12

if score > 10 {
  print("high")
} else {
  print("low")
}
```

```
high
```

---

## Error Messages

```
TypeError at line 1, column 5: variable 'x' declared as Number but initializer has type Text
TypeError at line 3, column 12: function 'add' argument 2 expected Number but got Text
TypeError at line 2, column 3: function declared return type Number but returned Text
TypeError at line 1, column 1: function 'add' expected 2 arguments but got 1
TypeError at line 1, column 1: cannot call 'x': value has type Number, not Function
TypeError: if condition must be Bool, got Number
TypeError at line 3, column 5: cannot add meters and seconds
TypeError at line 2, column 5: variable 'bad' declared as seconds but initializer has type meters
TypeError at line 1, column 5: cannot add meters/seconds and meters
TypeError at line 4, column 5: variable 'v' declared as meters but initializer has type meters/seconds
ParseError at line 2, column 5: expected expression
LexError at line 3, column 7: unexpected character '@'
```

---

## Tests

```sh
cargo test
```

443 tests pass as of Milestone 8B (post-audit).

---

## Project Structure

```
src/
  main.rs         CLI entry point (clap) — run / check / repl / bytecode subcommands
  lib.rs          Module declarations + tests
  token.rs        Token types and Span
  lexer.rs        Source → tokens
  ast.rs          Expression and statement AST nodes (includes TypeAnnotation, Param)
  parser.rs       Recursive-descent parser (includes resolve_unit registry)
  typechecker.rs  Static type checker (TypeEnv, TypeChecker, Type, NumberWithUnit)
  value.rs        Runtime value enum (includes FunctionValue)
  env.rs          Lexical scope chain (Rc<RefCell<Env>>)
  interpreter.rs  Tree-walk interpreter
  error.rs        Structured error types (KiminError wraps Lex/Parse/Type/Runtime/Compile)
  repl.rs         Interactive REPL
  bytecode.rs     Instruction enum, Constant, Chunk, FunctionChunk, BytecodeProgram
  compiler.rs     BytecodeCompiler — lowers AST to bytecode; function chunks + named calls
  disassemble.rs  Human-readable bytecode listing printer (main + function chunks)
  tests.rs        Unit tests (443 tests)
examples/
  hello.kimin
  arithmetic.kimin
  variables.kimin
  conditionals.kimin
  blocks.kimin
  errors.kimin
  functions.kimin
  return.kimin
  recursion.kimin
  function_errors.kimin
  lexical_scoping.kimin
  closure.kimin
  types.kimin
  typed_functions.kimin
  type_errors.kimin
  units.kimin
  unit_functions.kimin
  unit_errors.kimin
  compound_units.kimin
  compound_unit_errors.kimin
  states.kimin
  state_errors.kimin
  state_functions.kimin
  simulate.kimin
  simulate_state.kimin
  simulate_errors.kimin
  simulate_time_units.kimin
  simulate_time_unit_errors.kimin
  mutable.kimin
  mutable_units.kimin
  mutable_errors.kimin
  simulate_motion.kimin
  bytecode_demo.kimin
  bytecode_functions.kimin
```

---

## Known Limitations

- No anonymous functions / lambda syntax
- No multiline REPL — function declarations must fit on one input line in the REPL
- `print` is a statement keyword, not a user-definable function
- No variable assignment after declaration (`let` only; no `x = 5`)
- `RuntimeError` has no source location yet (spans planned for a future milestone)
- Units are static-only in M4 — no runtime unit tracking or unit conversion
- No derived unit simplification (`kg*m/s²` does not automatically reduce to `newtons`)
- No compound unit annotations in source — compound types are inferred only; you cannot write `let v: meters/seconds = ...`
- State transitions inside function bodies modify the function's local copy, not the caller's variable
- No state transition guards or entry/exit actions
- No automatic or event-driven transitions
- No SI prefixes (`km`, `MHz` are not recognized)
- No `5 meters` expression-literal syntax — units can only appear as type annotations
- No unit conversion between time units — `minutes` and `seconds` are distinct, non-interchangeable types
- `simulate` body type-checked once; known-variant tracking after transitions inside the body does not carry across iterations statically
- No compound assignment operators (`+=`, `-=`, `*=`, `/=`)
- No mutable function parameters — parameters are always immutable
- No general loops (`while`, `for`) — only `simulate` blocks
- Bytecode IR: function declarations and named calls now lower fully; state machines, `transition`, and `simulate` still emit `Unsupported(...)` markers
- Bytecode IR: dynamic/computed calls (`get_fn()(args)`) emit `UNSUPPORTED(dynamic call)` — not yet supported
- Bytecode IR: free variable capture (closures) inside function bodies emits `LOAD_GLOBAL` provisionally — not semantically correct for all closure patterns
- Bytecode IR is not executed — tree-walk interpreter remains the runtime

---

## Roadmap

| Milestone | Focus | Status |
|-----------|-------|--------|
| 1 | Lexer, parser, AST, tree-walk interpreter, REPL, tests | ✓ done |
| 2A | Named functions, parameters, return, recursion | ✓ done |
| 2B | Closures and lexical scoping (`Rc<RefCell<Env>>` chain) | ✓ done |
| 3 | Static type checking | ✓ done |
| 4 | Unit-aware types (`let d: meters = 10`) | ✓ done |
| 4B | Compound unit inference (`meters / seconds → meters/seconds`) | ✓ done |
| 5 | State machines as first-class language constructs | ✓ done |
| 6A | `simulate` blocks with `seconds` time unit and `time` variable | ✓ done |
| 6B | Extended time units (`milliseconds`, `minutes`, `hours`) for `simulate` | ✓ done |
| 7A | `let mut` and type-safe assignment; mutable simulate accumulators | ✓ done |
| 8A | Flat bytecode IR emission (`kimin bytecode`); `Unsupported` markers for advanced features | ✓ done |
| 8B | Function chunks and named call lowering in bytecode IR | ✓ done |
| 8 | Full bytecode VM execution; state/simulate lowering | planned |
