# Kimin Language Specification — Milestone 3

This document describes the syntax and semantics implemented through Milestone 3.

---

## 1. Lexical Structure

### 1.1 Comments

Line comments start with `//` and extend to end of line.

```kimin
// This is a comment
let x = 1  // inline comment
```

### 1.2 Whitespace

Spaces, tabs, carriage returns, and newlines are all treated as whitespace and ignored by the lexer. Newlines do not act as statement terminators (no semicolon insertion).

### 1.3 Literals

**Numbers** — integer and floating-point:

```kimin
42
3.14
0.5
```

Internally stored as `f64`. Whole numbers are printed without a decimal point.

**Strings** — double-quoted, single-line:

```kimin
"hello"
"Matthew"
```

**Booleans:**

```kimin
true
false
```

### 1.4 Identifiers

Start with a letter or `_`, followed by letters, digits, or `_`.

```
foo  bar_baz  _x  score1
```

### 1.5 Keywords

```
let  if  else  print  fn  return  true  false
```

### 1.6 Operators and Delimiters

| Token | Meaning |
|-------|---------|
| `+`   | Addition / string concatenation |
| `-`   | Subtraction / unary negation |
| `*`   | Multiplication |
| `/`   | Division |
| `!`   | Logical NOT |
| `==`  | Equality |
| `!=`  | Inequality |
| `<`   | Less than |
| `<=`  | Less than or equal |
| `>`   | Greater than |
| `>=`  | Greater than or equal |
| `=`   | Assignment (in `let`) |
| `:`   | Type annotation separator |
| `->`  | Return type annotation |
| `(`   | Open group |
| `)`   | Close group |
| `{`   | Open block |
| `}`   | Close block |
| `,`   | Parameter / argument separator |

---

## 2. Types

### 2.1 Runtime types

| Type     | Examples            | Notes |
|----------|---------------------|-------|
| Number   | `42`, `3.14`        | IEEE 754 `f64` |
| Text     | `"hello"`           | UTF-8 string |
| Bool     | `true`, `false`     | |
| Nil      | (runtime only)      | No literal syntax |
| Function | (runtime only)      | `FunctionValue` in the interpreter |

### 2.2 Static types (Milestone 3)

The static type checker uses the same names as the runtime types. Type annotations are written as `Number`, `Text`, `Bool`, or `Nil`.

Functions without a return type annotation are assigned `Unknown` — the gradual-typing escape hatch. Operations involving an `Unknown` value propagate `Unknown` without error, so unannotated code remains valid.

---

## 3. Expressions

### 3.1 Precedence (low → high)

| Level | Operators        |
|-------|-----------------|
| 1 (lowest) | `==`, `!=` |
| 2 | `<`, `<=`, `>`, `>=` |
| 3 | `+`, `-` |
| 4 | `*`, `/` |
| 5 | Unary `-`, `!` |
| 6 (highest) | Literals, variables, grouping, calls |

### 3.2 Arithmetic

```kimin
1 + 2 * 3    // 7  (multiplication before addition)
(1 + 2) * 3  // 9
-5           // unary negation
```

String concatenation uses `+`:

```kimin
"hello" + " world"  // "hello world"
```

Static rule: `+` requires `Number + Number` or `Text + Text`. Mixing types is a `TypeError`.

### 3.3 Comparisons

```kimin
score > 10
x == 5
name != "error"
```

All comparison operators return `Bool`.

Static rule: `<`, `<=`, `>`, `>=` require `Number` operands. `==` and `!=` require both operands to be the same type.

### 3.4 Unary Operators

```kimin
-x       // numeric negation; requires Number
!cond    // logical NOT; requires Bool
```

Static rule: `-` requires `Number`; `!` requires `Bool`.

### 3.5 Variables

```kimin
score
name
```

Reading an undefined variable is a `TypeError`.

### 3.6 Grouping

```kimin
(1 + 2) * 3
```

---

## 4. Statements

Programs are sequences of statements. No semicolons required.

### 4.1 Variable Declaration

```kimin
let <name> = <expr>
let <name>: <type> = <expr>
```

Declares `<name>` in the current scope. The type annotation is optional. When present, the type checker verifies that the initializer expression matches the declared type.

```kimin
let x = 10                  // type inferred as Number
let name: Text = "Matthew"  // annotation checked against initializer
let flag: Bool = true
```

Static rule: if an annotation is present and the initializer has a different concrete type, a `TypeError` is raised.

### 4.2 Print

```kimin
print(<expr>)
```

Evaluates `<expr>` and writes it to stdout followed by a newline. `print` is a statement keyword, not a user-definable function. Printing a `Function` value is a `TypeError`.

### 4.3 Block

```kimin
{
  <stmt>*
}
```

Creates a new lexical scope. Variables declared inside the block are not visible outside.

### 4.4 If / Else

```kimin
if <expr> {
  <stmts>
}

if <expr> {
  <stmts>
} else {
  <stmts>
}
```

Static rule: the condition must have type `Bool` (or `Unknown`).

```kimin
if score > 10 {
  print("high")
} else {
  print("low")
}
```

### 4.5 Expression Statement

Any expression used as a statement; its value is discarded.

---

## 5. Scoping Rules

Kimin uses **lexical (static) scoping**:

- Each `{...}` block creates a new scope.
- A name lookup walks from the innermost scope outward.
- Variables in inner scopes **shadow** outer ones without mutating them.
- Variables do not leak out of the block they were declared in.

---

## 5B. Functions

### 5B.1 Function Declarations

```kimin
fn name(param1: Type1, param2: Type2) -> ReturnType {
  statements
}
```

- Parameters require type annotations (e.g., `: Number`).
- The return type annotation (`-> Type`) is optional. When omitted, the return type is `Unknown` (gradual typing).
- Zero parameters are allowed: `fn greet() { ... }`.
- Declared at any statement position (top level or inside a block).
- Binds the function name in the current scope as a `Function` value.

```kimin
fn add(a: Number, b: Number) -> Number {
  return a + b
}

fn greet(name: Text) -> Text {
  return "Hello, " + name
}

fn zero() -> Number {
  return 0
}
```

### 5B.2 Function Calls

```kimin
name(arg1, arg2)
name()
add(1, 2)
square(add(2, 3))
```

Static rules:
- Wrong arity: `TypeError: function 'add' expected 2 arguments but got 1`
- Wrong argument type: `TypeError: function 'add' argument 2 expected Number but got Text`
- Non-function callee: `TypeError: cannot call 'x': value has type Number, not Function`

### 5B.3 Return Statement

```kimin
return expr
return
```

- `return expr` exits the current function and yields `expr` as the call result.
- Bare `return` exits and yields `nil`.
- A function that falls off the end without `return` yields `nil`.
- `return` propagates through nested blocks and `if` statements until it exits the function.

Static rules:
- `return` at top level (outside any function): `TypeError: cannot return outside of a function`
- Return value type must match declared return type when both are known: `TypeError: function declared return type Number but returned Text`

### 5B.4 Recursion

The function name is bound before the body executes, so recursive calls are visible.

```kimin
fn fact(n: Number) -> Number {
  if n <= 1 {
    return 1
  }
  return n * fact(n - 1)
}

print(fact(5))  // 120
```

The type checker pre-registers all function signatures in a two-pass scan, so mutual recursion type-checks correctly regardless of declaration order.

### 5B.5 Lexical Scoping and Closures

Functions capture their enclosing environment at declaration time.

```kimin
let x = 10
fn show() -> Number { return x }
fn caller() {
  let x = 99
  return show()   // returns 10, not 99
}
print(caller())   // 10
```

Closures keep their environment alive after the enclosing function returns:

```kimin
fn make_getter() {
  let x = 77
  fn get() { return x }
  return get
}
let getter = make_getter()
print(getter())   // 77
```

---

## 6. Static Type Checker (Milestone 3)

The type checker runs as a separate pass between the parser and the interpreter.

### 6.1 Type rules summary

| Construct | Rule |
|-----------|------|
| `let x: T = e` | `e` must have type `T` |
| `let x = e` | type inferred from `e` |
| `fn f(p: T) -> R` | body must return `R`; args must match param types |
| `if cond` | `cond` must be `Bool` |
| `!x` | `x` must be `Bool` |
| `-x` | `x` must be `Number` |
| `a + b` | both `Number`, or both `Text` |
| `a - b`, `a * b`, `a / b` | both `Number` |
| `a < b`, `a <= b`, `a > b`, `a >= b` | both `Number` |
| `a == b`, `a != b` | both same type |

### 6.2 Gradual typing

Functions without a return type annotation get return type `Unknown`. Any operation on an `Unknown` value propagates `Unknown` without error. This lets unannotated functions coexist with typed ones.

```kimin
fn no_ret(x: Number) {
  print(x)   // ok — return type is Unknown (nil at runtime)
}
```

---

## 7. Errors

All errors include a phase name and, where possible, source location.

### 7.1 Lex Errors

```
LexError at line 3, column 7: unexpected character '@'
LexError at line 5, column 1: unterminated string literal
```

### 7.2 Parse Errors

```
ParseError at line 2, column 5: expected expression
ParseError at line 1, column 5: expected identifier after 'let'
ParseError at line 4, column 3: expected '}'
ParseError at line 1, column 8: expected ':' after parameter name (parameters require type annotations)
ParseError at line 1, column 10: unknown type 'Numbr'; expected Number, Text, Bool, or Nil
```

### 7.3 Type Errors

```
TypeError at line 1, column 5: variable 'x' declared as Number but initializer has type Text
TypeError at line 3, column 12: function 'add' argument 2 expected Number but got Text
TypeError at line 2, column 3: function declared return type Number but returned Text
TypeError at line 1, column 1: function 'add' expected 2 arguments but got 1
TypeError at line 1, column 1: cannot call 'x': value has type Number, not Function
TypeError: if condition must be Bool, got Number
TypeError: unary '!' requires Bool, got Number
TypeError: operator '+' expected Number + Number or Text + Text, got Number + Text
TypeError: operator '==' requires same-type operands, got Number and Text
TypeError: undefined variable 'x'
TypeError: cannot return outside of a function
```

### 7.4 Runtime Errors

```
RuntimeError: division by zero
```

Most errors that were previously RuntimeErrors (undefined variable, wrong arity, etc.) are now TypeErrors caught before execution.

---

## 8. Grammar (EBNF)

```
program      = stmt* EOF
stmt         = fn_decl | return_stmt | let_stmt | print_stmt | if_stmt | block | expr_stmt
fn_decl      = "fn" IDENT "(" params ")" ("->" type_ann)? fn_body
return_stmt  = "return" expr?
let_stmt     = "let" IDENT (":" type_ann)? "=" expr
print_stmt   = "print" "(" expr ")"
if_stmt      = "if" expr block ("else" block)?
block        = "{" stmt* "}"
fn_body      = "{" stmt* "}"
params       = (typed_param ("," typed_param)*)?
typed_param  = IDENT ":" type_ann
type_ann     = "Number" | "Text" | "Bool" | "Nil"
expr_stmt    = expr

expr         = equality
equality     = comparison (("==" | "!=") comparison)*
comparison   = term (("<" | "<=" | ">" | ">=") term)*
term         = factor (("+" | "-") factor)*
factor       = unary (("*" | "/") unary)*
unary        = ("-" | "!") unary | call
call         = primary ("(" args ")")*
primary      = NUMBER | STRING | "true" | "false" | IDENT | "(" expr ")"
args         = (expr ("," expr)*)?
```
