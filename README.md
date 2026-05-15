# Kimin

An experimental programming language designed by Matthew Kim. Kimin is being built as a modern systems/engineering language where **units, time, state, and constraints** will eventually become first-class language features.

This repository contains **Milestone 4**: unit-aware static types. Built on top of the Milestone 3 static type checker.

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
  - `meters * seconds → TypeError` (compound units not supported yet)
  - `meters / meters → Number` (same-unit division gives dimensionless result)
  - `meters / seconds → TypeError` (derived units not supported yet)
- Number literals promote to unit type at assignment (`let d: meters = 10` is valid)
- Unit-typed function parameters and return types: `fn add_dist(a: meters, b: meters) -> meters`
- Number literals promote in function call arguments (`add_dist(10, 5)` is valid)

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
TypeError at line 3, column 5: compound units not supported yet: meters * seconds
ParseError at line 2, column 5: expected expression
LexError at line 3, column 7: unexpected character '@'
```

---

## Tests

```sh
cargo test
```

146 tests pass as of Milestone 4.

---

## Project Structure

```
src/
  main.rs         CLI entry point (clap)
  lib.rs          Module declarations + tests
  token.rs        Token types and Span
  lexer.rs        Source → tokens
  ast.rs          Expression and statement AST nodes (includes TypeAnnotation, Param)
  parser.rs       Recursive-descent parser (includes resolve_unit registry)
  typechecker.rs  Static type checker (TypeEnv, TypeChecker, Type, NumberWithUnit)
  value.rs        Runtime value enum (includes FunctionValue)
  env.rs          Lexical scope chain (Rc<RefCell<Env>>)
  interpreter.rs  Tree-walk interpreter
  error.rs        Structured error types (KiminError wraps Lex/Parse/Type/Runtime)
  repl.rs         Interactive REPL
  tests.rs        Unit tests (146 tests)
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
```

---

## Known Limitations

- No anonymous functions / lambda syntax
- No multiline REPL — function declarations must fit on one input line in the REPL
- `print` is a statement keyword, not a user-definable function
- No variable assignment after declaration (`let` only; no `x = 5`)
- `RuntimeError` has no source location yet (spans planned for a future milestone)
- Units are static-only in M4 — no runtime unit tracking or unit conversion
- No compound unit algebra (`meters * seconds`, `meters / seconds` are TypeErrors)
- No derived units (`N`, `J`, `W` are in the registry as annotations but `newtons = kg * m/s²` is not computed)
- No SI prefixes (`km`, `ms`, `MHz` are not recognized)
- No `5 meters` expression-literal syntax — units can only appear as type annotations

---

## Roadmap

| Milestone | Focus | Status |
|-----------|-------|--------|
| 1 | Lexer, parser, AST, tree-walk interpreter, REPL, tests | ✓ done |
| 2A | Named functions, parameters, return, recursion | ✓ done |
| 2B | Closures and lexical scoping (`Rc<RefCell<Env>>` chain) | ✓ done |
| 3 | Static type checking | ✓ done |
| 4 | Unit-aware types (`let d: meters = 10`) | ✓ done |
| 5 | State machines as first-class language constructs | planned |
| 6 | Time blocks and simulation primitives | planned |
| 7 | Bytecode / IR, potential WASM target | planned |
