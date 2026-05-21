<div align="center">

# ⚡ Kimin

**An experimental systems and engineering language built from scratch in Rust**

*Physical units &nbsp;·&nbsp; State machines &nbsp;·&nbsp; Deterministic simulation — as first-class type system features*

![Tests](https://img.shields.io/badge/tests-2234_passing-4caf50?style=flat-square)
![Rust](https://img.shields.io/badge/rust-2021_edition-orange?style=flat-square&logo=rust)
![Status](https://img.shields.io/badge/status-experimental-blue?style=flat-square)
![Milestone](https://img.shields.io/badge/milestone-11E-informational?style=flat-square)

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

This is a from-scratch implementation: hand-written lexer, recursive-descent parser, static type checker, tree-walk interpreter, bytecode compiler, and stack-based VM — all in Rust, ~15k lines, **2234 tests passing**.

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

### Loops and loop control

**While loops:**
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

**For/range loops:**
```
// Prints 0 1 2 3 4
for i in range(0, 5) {
    print(i)
}

// Sum 1 through 10
let mut total: Number = 0
for i in range(1, 11) {
    total += i
}
print(total)  // 55
```

- `while`: condition must be `Bool`; any other type → TypeError
- `for i in range(start, end)`: iterates `i` from `start` (inclusive) to `end` (exclusive) by 1
  - `start` and `end` must be `Number`; loop variable is immutable and loop-local
  - Zero iterations if `start >= end`
- `break`: exits the nearest enclosing loop
- `continue`: skips the rest of the body; in for loops, jumps to the increment step
- Both target the **nearest** enclosing loop only; no labels
- Neither crosses function or simulate boundaries (loop context resets on entry)

### Arrays

```kimin
let primes = [2, 3, 5, 7, 11]
print(primes[0])       // 2
print(len(primes))     // 5
print(primes[1..4])    // [3, 5, 7]

let mut sum = 0
for i in range(0, len(primes)) {
    sum += primes[i]
}
print(sum)             // 28
```

- Array literal `[e1, e2, e3]` — all elements must share the same type
- **Explicit array type annotation** `Array<T>` — valid in `let` declarations, function parameters, and return types
  - `T` can be `Number`, `Text`, `Bool`, a unit name (`meters`, `seconds`, …), or a state machine name
  - Nested arrays (`Array<Array<T>>`) are not supported
- **Empty array literal `[]`** — allowed only when the expected type is known from an annotation or return type
  - `let nums: Array<Number> = []` — OK
  - `let nums = []` — TypeError (no element type can be inferred)
  - `fn make() -> Array<Number> { return [] }` — OK
- Index expression `arr[i]` — `i` must be a `Number`; runtime checks: integer, non-negative, in-bounds
- Slice expression `arr[start..end]` — returns a new independent `Array<T>` from `start` inclusive to `end` exclusive
  - `start` and `end` must be `Number`; runtime checks: integer, non-negative, `start <= end`, `end <= len(arr)`
  - Slices can be empty (`arr[2..2]`)
  - Slice assignment, open-ended slices (`arr[1..]`, `arr[..3]`), and step slices are not supported
- `len(arr)` builtin — returns the number of elements as `Number`
- **Index assignment `arr[i] = value`** — requires `let mut` array; element type must match; array stays fixed-size
  - Runtime: integer index, non-negative, in-bounds; updates binding via `assign_existing`
  - Bytecode: `SET_INDEX name` instruction pops index and value; looks up array in env chain
- **Index compound assignment `arr[i] op= value`** — requires `let mut` array; index evaluated once
  - Supported operators: `+=`, `-=`, `*=`, `/=`
  - Semantics: sugar for `arr[i] = arr[i] op value`, but the current element is read internally and the index is not re-evaluated
  - Bytecode: `INDEX_COMPOUND_ASSIGN name op` updates the existing array binding through the env chain

```kimin
let mut nums = [1, 2, 3]
nums[0] = 99
for i in range(0, len(nums)) {
    nums[i] = nums[i] * 2
}
print(nums[0])   // 198
print(nums[1])   // 4
print(nums[2])   // 6
```

- **`push(arr, value)` builtin** — appends `value` to a mutable array; returns `Nil`; first argument must be a mutable `Array<T>` variable; value type must match element type (Number→unit promotion allowed)
- **`pop(arr)` builtin** — removes and returns the last element of a mutable array; RuntimeError if empty; return type is the element type `T`

```kimin
let mut log = [0]
push(log, 1)
push(log, 2)
print(len(log))   // 3

let v = pop(log)
print(v)          // 2
print(len(log))   // 2
```

Slices are copies, not mutation views:

```kimin
let mut nums = [1, 2, 3, 4]
let mut middle = nums[1..3]

middle[0] = 99
nums[2] = 88

print(middle[0])  // 99
print(middle[1])  // 3
print(nums[1])    // 2
print(nums[2])    // 88
```

### String indexing and slicing

- `len(s)` — returns character count as `Number` (Unicode scalar values / Rust `char`s)
- `s[i]` — returns a one-character `Text` at char index `i`
- `s[start..end]` — returns a `Text` substring, char-indexed, end-exclusive

```kimin
let s = "hello"
print(len(s))     // 5
print(s[0])       // h
print(s[1..4])    // ell

fn first(t: Text) -> Text {
  return t[0]
}
print(first("world"))   // w
```

Indexing is by Unicode scalar value (Rust `char`), not bytes. Strings are immutable: `s[i] = "x"`, `push(s, "a")`, and `pop(s)` are all unsupported.

### String utility builtins

- `contains(text, pattern) -> Bool` — true if pattern appears anywhere in text
- `starts_with(text, prefix) -> Bool` — true if text begins with prefix
- `ends_with(text, suffix) -> Bool` — true if text ends with suffix

Empty pattern/prefix/suffix always returns `true`.

### String transformation builtins

- `to_upper(text) -> Text` — uppercase; delegates to Rust `String::to_uppercase`
- `to_lower(text) -> Text` — lowercase; delegates to Rust `String::to_lowercase`
- `trim(text) -> Text` — removes leading and trailing Unicode whitespace; delegates to Rust `str::trim`

All three accept exactly one `Text` argument and return `Text`. Unicode case conversion follows Rust's Unicode rules (e.g., `to_upper("éclair")` → `"ÉCLAIR"`). String length may change for some codepoints during case conversion.

```kimin
print(to_upper("hello"))                          // HELLO
print(to_lower("HELLO"))                          // hello
print(trim("  hello  "))                          // hello
print(contains(to_lower("HELLO WORLD"), "world")) // true
```

### String split builtin

- `split(text, delimiter) -> Array<Text>` — split a string by a delimiter; returns an `Array<Text>`

Empty delimiter splits the string into individual characters. Consecutive delimiters produce empty-string elements. Delimiter not found returns a single-element array containing the original string.

```kimin
let parts = split("a,b,c", ",")
print(len(parts))       // 3
print(parts[0])         // a

let chars = split("abc", "")
print(len(chars))       // 3
print(chars[0])         // a

print(to_upper(split("hello world", " ")[0]))  // HELLO
```

### String join builtin

- `join(parts, delimiter) -> Text` — join an `Array<Text>` with a delimiter; returns `Text`

Empty delimiter concatenates elements directly. Empty array returns `""`. Single-element array returns that element unchanged.

```kimin
let words: Array<Text> = ["hello", "world", "kimin"]
print(join(words, ", "))          // hello, world, kimin

let csv = join(["one", "two", "three"], ",")
print(csv)                        // one,two,three

// Round-trip with split
let parts = split("a-b-c", "-")
print(join(parts, "::"))          // a::b::c
```

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
TypeError at line 4, col 1:  'break' used outside of a loop
TypeError:                   while condition must be Bool, got Number
ParseError at line 2, col 5: expected expression
LexError  at line 3, col 7:  unexpected character '@'
```

---

## ⚠️ Known Limitations

| Limitation | Notes |
|---|---|
| No anonymous functions | No lambda syntax |
| No labeled `break`/`continue` | Targets nearest enclosing while/for only; no label syntax |
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
| No slice mutation views | `arr[start..end]` returns a new array copy; mutating the slice never mutates the source |
| No slice assignment | `arr[1..3] = value` and `arr[1..3] += value` are unsupported |
| No open-ended or stepped slices | `arr[1..]`, `arr[..3]`, and `arr[1..5..2]` are unsupported |
| No string mutation | `str[i] = value`, `push(str, c)`, `pop(str)` unsupported |
| No open-ended or stepped string slices | `str[1..]`, `str[..3]`, `str[1..5..2]` unsupported |
| String indexing is char-based | Indexes Unicode scalar values (Rust `char`s), not grapheme clusters |
| No nested arrays | `Array<Array<T>>` is a ParseError; one level of array nesting only |
| No array type annotation without `<T>` | `let a: Array = [1,2]` is a ParseError — `Array` must always have an element type |
| `len`/`push`/`pop` shadow user functions | These builtins take precedence over any user-defined functions with those names |
| `time` in simulate has unit type | `time` cannot be used as an array index; use an outer mutable counter instead |
| No mixed semantics for state arrays | `arr[i] += value` is arithmetic/string-only; state arrays still need direct replacement like `arr[i] = Door.open` |

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
| 9D | `for i in range(start, end)` loops | ✅ |
| 9E | Fixed-size typed arrays (`[e1,e2]`, `arr[i]`, `len`) | ✅ |
| 10A | Array mutation by index (`arr[i] = value`, `let mut` required) | ✅ |
| 10B | Array index compound assignment (`arr[i] += value`, etc.) | ✅ |
| 10C | `push(arr, value)` and `pop(arr)` builtins | ✅ |
| 10D | Array slice expressions (`arr[start..end]`) | ✅ |
| 10E | Explicit `Array<T>` type annotations; empty array literals with annotation | ✅ |
| 10F | Expected-type propagation for call arguments; `f([])` works when param is `Array<T>` | ✅ |

---

## 🧪 Tests

```sh
cargo test
# 2234 passed, 0 failed
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
  value.rs        Runtime values: Number, Text, Bool, Nil, Function, StateValue, BytecodeFunction, Array
  env.rs          Lexical scope chain — Rc<RefCell<Env>>
  interpreter.rs  Tree-walk interpreter (ExecFlow: Normal / Return / Break / Continue)
  error.rs        Structured errors: Lex / Parse / Type / Runtime / Compile
  repl.rs         Interactive REPL with persistent type checker and interpreter
  bytecode.rs     Instruction enum, Chunk, FunctionChunk, SimulateChunk, BytecodeProgram
  compiler.rs     BytecodeCompiler — AST → flat bytecode (LoopContext for break/continue patching)
  disassemble.rs  Human-readable bytecode listing printer
  vm.rs           Stack-based VM — env-chain model, execute_chunk
  lib.rs          Module declarations
  tests.rs        2234 unit tests
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
  break_continue_errors.kimin       for_range.kimin
  for_range_sum.kimin               for_range_break_continue.kimin
  for_range_function.kimin          for_range_errors.kimin
  arrays.kimin                      arrays_loop.kimin
  arrays_units.kimin                array_errors.kimin
  array_mutation.kimin              array_mutation_loop.kimin
  array_mutation_simulate.kimin     array_mutation_errors.kimin
  array_index_compound.kimin        array_index_compound_loop.kimin
  array_index_compound_simulate.kimin  array_index_compound_errors.kimin
  array_push_pop.kimin              array_push_pop_loop.kimin
  array_push_pop_simulate.kimin     array_push_pop_errors.kimin
  array_slices.kimin                array_slices_loop.kimin
  array_slices_mutation.kimin       array_slices_errors.kimin
  array_annotations.kimin           array_annotations_push.kimin
  array_annotations_function.kimin  array_annotations_units.kimin
  array_annotations_errors.kimin
  array_call_expected.kimin         array_call_expected_units.kimin
  array_call_expected_errors.kimin
  strings.kimin                     strings_loop.kimin
  strings_functions.kimin           strings_simulate.kimin
  string_errors.kimin
  string_utils.kimin                string_utils_functions.kimin
  string_utils_loop.kimin           string_utils_unicode.kimin
  string_utils_errors.kimin
  string_transforms.kimin           string_transforms_functions.kimin
  string_transforms_loop.kimin      string_transforms_unicode.kimin
  string_transforms_errors.kimin
  string_split.kimin                string_split_empty_delim.kimin
  string_split_loop.kimin           string_split_functions.kimin
  string_split_simulate.kimin       string_split_errors.kimin
  string_join.kimin                 string_join_split.kimin
  string_join_loop.kimin            string_join_functions.kimin
  string_join_simulate.kimin        string_join_errors.kimin
  bytecode_demo.kimin               bytecode_functions.kimin
  vm_demo.kimin                     vm_recursion.kimin
  vm_simulate_state.kimin           vm_closure_capture.kimin
  vm_dynamic_calls.kimin            vm_dynamic_adder.kimin
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

### Milestone 9D
- **For/range loops** (`for i in range(start, end) { ... }`)
  - Iterates `i` from `start` (inclusive) to `end` (exclusive) by 1; zero iterations if `start >= end`
  - Loop variable is immutable and loop-local; `start`/`end` must be `Number`
  - `break` and `continue` work inside for loops; `continue` jumps to the increment step (not the condition)
  - Bytecode: outer `BeginScope` holds loop var + sentinel; increment emitted after body; no new VM instructions

### Milestone 9E
- **Fixed-size typed arrays** (`[e1, e2, e3]`, `arr[i]`, `len(arr)`)
  - Array literal must have at least one element; all elements must share the same type (otherwise TypeError)
  - Index expression `arr[i]`: index must be `Number`; runtime bounds/integer/negative checks enforced
  - `len(arr)` builtin: returns element count as `Number`; intercepted before normal function dispatch
  - Bytecode: `Array { count }`, `Index`, `Len` instructions; `len` compiles to `Len` (no Call emitted)
  - Both `kimin run` and `kimin vm` support all array operations

### Milestone 10A
- **Array mutation by index** (`arr[i] = value`)
  - Requires a `let mut` array binding; immutable arrays reject index assignment statically
  - Index must type-check as `Number`; runtime still enforces integer, non-negative, and in-bounds access
  - Assigned value must match the array element type; `Number` still promotes into unit-typed element slots
  - Works in functions, closures, `for` loops, and `simulate` bodies through env-chain reassignment
  - Bytecode: `SetIndex(name)` / `SET_INDEX name` updates the existing array binding in place

### Milestone 10B
- **Array index compound assignment** (`arr[i] += value`, `arr[i] *= value`, etc.)
  - Supported operators: `+=`, `-=`, `*=`, `/=`
  - Requires a `let mut` array binding; immutable arrays reject index compound assignment statically
  - Index must type-check as `Number`; runtime still enforces integer, non-negative, and in-bounds access
  - The element type participates in the same binary operator rules as normal compound assignment
  - Index is evaluated once, then the current element is read, combined with the rhs, and written back
  - Works in functions, closures, `for` loops, and `simulate` bodies through env-chain reassignment
  - Bytecode: `IndexCompoundAssign { name, op }` / `INDEX_COMPOUND_ASSIGN name op`

### Milestone 10C
- **`push(arr, value)` and `pop(arr)` builtins**
  - Both require a mutable array variable as the first argument; expressions like `push(a[0..2], 9)` are rejected
  - `push` appends and returns `Nil`; `pop` removes and returns the last element
  - Bytecode: `ArrayPush(name)` / `ARRAY_PUSH name` and `ArrayPop(name)` / `ARRAY_POP name`

### Milestone 10D
- **Array slice expressions** (`arr[start..end]`)
  - Static rules: target must be `Array<T>`; both bounds must be `Number`; result type is `Array<T>`
  - Runtime rules: bounds must be integer-like and non-negative, `start <= end`, and `end <= len(arr)`
  - Slices are end-exclusive and return independent array copies
  - Bytecode: `Slice` / `SLICE` pops array, start, and end, then pushes the new array

</details>
