# Kimin

An experimental programming language designed by Matthew Kim. Kimin is being built as a modern systems/engineering language where **units, time, state, and constraints** will eventually become first-class language features.

This repository contains **Milestone 1**: a working tree-walk interpreter written in Rust.

---

## Milestone 1 Features

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
cargo run -- run examples/hello.kimin
```

### Check syntax (no execution)

```sh
cargo run -- check examples/conditionals.kimin
```

### Start the REPL

```sh
cargo run -- repl
```

---

## Examples

### hello.kimin

```kimin
print("Hello from Kimin")
```

```
Hello from Kimin
```

### arithmetic.kimin

```kimin
print(1 + 2 * 3)
print((1 + 2) * 3)
```

```
7
9
```

### variables.kimin

```kimin
let name = "Matthew"
let score = 42
print(name)
print(score)
```

```
Matthew
42
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

### blocks.kimin

```kimin
let x = 5

{
  let inner = 99
  print(inner)  // 99
  print(x)      // 5
}

print(x)  // 5 — inner is gone, x unchanged
```

---

## Error Messages

```
RuntimeError: undefined variable 'z'
RuntimeError: cannot add Number and Bool
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
  value.rs        Runtime value enum
  env.rs          Lexical scope stack
  interpreter.rs  Tree-walk interpreter
  error.rs        Structured error types
  repl.rs         Interactive REPL
  tests.rs        Unit tests
examples/
  hello.kimin
  arithmetic.kimin
  variables.kimin
  conditionals.kimin
  blocks.kimin
  errors.kimin
```

---

## Roadmap

| Milestone | Focus |
|-----------|-------|
| 1 (now) | Lexer, parser, AST, tree-walk interpreter, REPL, tests |
| 2 | Functions, return values, closures |
| 3 | Static type checking |
| 4 | Unit-aware types (`5 meters`, `10 kg`) |
| 5 | State machines as first-class language constructs |
| 6 | Time blocks and simulation primitives |
| 7 | Bytecode / IR, potential WASM target |
