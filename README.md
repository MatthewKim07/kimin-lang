# Kimin

An experimental programming language designed by Matthew Kim. Kimin is being built as a modern systems/engineering language where **units, time, state, and constraints** will eventually become first-class language features.

This repository contains **Milestone 2B**: closures and lexical scoping. Built on top of the Milestone 2A tree-walk interpreter.

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
- Runtime errors for wrong arity and non-function calls

### Milestone 2B
- Lexical (static) scoping — functions see variables from their definition site, not the call site
- True closures — functions capture their enclosing environment at declaration time
- Nested functions that capture outer locals
- Returned functions that keep their captured environment alive after the enclosing function returns
- Mutual recursion still works (both names in global env before either is called)

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

### Check syntax (no execution)

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
fn add(a, b) {
  return a + b
}

fn square(x) {
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

### return.kimin — early return

```kimin
fn early(x) {
  if x > 10 {
    return "large"
  }
  return "small"
}

print(early(12))
print(early(3))
```

```
large
small
```

### recursion.kimin

```kimin
fn fact(n) {
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
RuntimeError: undefined variable 'x'
RuntimeError: function 'add' expected 2 arguments but got 1
RuntimeError: attempted to call non-function value Number
RuntimeError: cannot return outside of a function
ParseError at line 2, column 5: expected expression
```

---

## Tests

```sh
cargo test
```

---

## Project Structure

```
src/
  main.rs         CLI entry point (clap)
  lib.rs          Module declarations + tests
  token.rs        Token types and Span
  lexer.rs        Source → tokens
  ast.rs          Expression and statement AST nodes
  parser.rs       Recursive-descent parser
  value.rs        Runtime value enum (includes FunctionValue)
  env.rs          Lexical scope stack
  interpreter.rs  Tree-walk interpreter
  error.rs        Structured error types (KiminError)
  repl.rs         Interactive REPL
  tests.rs        Unit tests (82 tests)
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
```

---

## Known Limitations

- No anonymous functions / lambda syntax
- No static type checking — all types are dynamic
- No multiline REPL — function declarations must fit on one input line in the REPL
- `print` is a statement keyword, not a user-definable function
- No variable assignment after declaration (`let` only; no `x = 5`)

---

## Roadmap

| Milestone | Focus | Status |
|-----------|-------|--------|
| 1 | Lexer, parser, AST, tree-walk interpreter, REPL, tests | ✓ done |
| 2A | Named functions, parameters, return, recursion | ✓ done |
| 2B | Closures and lexical scoping (`Rc<RefCell<Env>>` chain) | ✓ done |
| 3 | Static type checking | planned |
| 4 | Unit-aware types (`5 meters`, `10 kg`) | planned |
| 5 | State machines as first-class language constructs | planned |
| 6 | Time blocks and simulation primitives | planned |
| 7 | Bytecode / IR, potential WASM target | planned |
