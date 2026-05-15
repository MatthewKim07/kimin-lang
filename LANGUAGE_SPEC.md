# Kimin Language Specification — Milestone 2A

This document describes the syntax and semantics implemented through Milestone 2A.

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
| `+`   | Addition |
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
| `(`   | Open group |
| `)`   | Close group |
| `{`   | Open block |
| `}`   | Close block |
| `,`   | Parameter / argument separator |

---

## 2. Types

| Type    | Examples            | Notes |
|---------|---------------------|-------|
| Number  | `42`, `3.14`        | IEEE 754 `f64` |
| String  | `"hello"`           | UTF-8 |
| Bool    | `true`, `false`     | |
| Nil     | (runtime only)      | No literal syntax in M1 |

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
| 6 (highest) | Literals, variables, grouping |

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

### 3.3 Comparisons

```kimin
score > 10
x == 5
name != "error"
```

All comparison operators return `Bool`.

### 3.4 Unary Operators

```kimin
-x       // numeric negation
!cond    // logical NOT; truthy = non-nil, non-false
```

### 3.5 Variables

```kimin
score
name
```

Reading an undefined variable is a `RuntimeError`.

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
```

Declares `<name>` in the current scope. Re-declaring in the same scope shadows the previous binding.

```kimin
let x = 10
let name = "Matthew"
let flag = true
```

### 4.2 Print

```kimin
print(<expr>)
```

Evaluates `<expr>` and writes it to stdout followed by a newline. `print` is a statement keyword, not a user-definable function.

```kimin
print("Hello from Kimin")
print(1 + 2)
print(x)
```

### 4.3 Block

```kimin
{
  <stmt>*
}
```

Creates a new lexical scope. Variables declared inside the block are not visible outside.

```kimin
let x = 5
{
  let inner = 99
  print(inner)  // ok
}
// print(inner)  // RuntimeError: undefined variable 'inner'
print(x)        // still 5
```

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

The condition is truthy if it is not `false` and not `nil`. Braces are required.

```kimin
if score > 10 {
  print("high")
} else {
  print("low")
}
```

### 4.5 Expression Statement

Any expression used as a statement; its value is discarded.

```kimin
1 + 1   // evaluated, result dropped
```

---

## 5. Scoping Rules

Kimin uses **lexical (static) scoping** for variables and blocks:

- Each `{...}` block creates a new scope.
- A name lookup walks from the innermost scope outward.
- Variables in inner scopes **shadow** outer ones without mutating them.
- Variables do not leak out of the block they were declared in.

---

## 5B. Functions (Milestone 2A)

### 5B.1 Function Declarations

```kimin
fn name(param1, param2) {
  statements
}
```

- Declared at any statement position (top level or inside a block).
- Parameters are identifiers, comma-separated. Zero parameters allowed.
- Binds the function name in the current scope as a `Function` value.

```kimin
fn greet(name) {
  print("Hello, " + name)
}

fn add(a, b) {
  return a + b
}

fn zero() {
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

- Call expressions have the highest expression precedence (postfix).
- Arguments are evaluated left to right before the body executes.
- Nested calls work: `square(add(2, 3))` → `square(5)` → `25`.
- Calling a non-function value: `RuntimeError: attempted to call non-function value Number`
- Wrong argument count: `RuntimeError: function 'add' expected 2 arguments but got 1`

### 5B.3 Return Statement

```kimin
return expr
return
```

- `return expr` exits the current function and yields `expr` as the call result.
- Bare `return` exits the function and yields `nil`.
- A function that falls off the end without a `return` also yields `nil`.
- `return` propagates through nested blocks and `if` statements until it exits the function.
- `return` at the top level (outside any function): `RuntimeError: cannot return outside of a function`

### 5B.4 Recursion

Recursive calls are supported. The function name is bound before the body executes, so it is visible in recursive calls.

```kimin
fn fact(n) {
  if n <= 1 {
    return 1
  }
  return n * fact(n - 1)
}

print(fact(5))  // 120
```

### 5B.5 Scoping Notes (Milestone 2A)

Function calls push a new scope on the existing scope stack. Parameters and locals are visible only within the call. The global scope (including function names) remains visible through the stack. Full closure capture is not yet implemented — see Milestone 2B.

---

## 6. Errors

All errors include a phase name and, where possible, source location.

### 6.1 Lex Errors

```
LexError at line 3, column 7: unexpected character '@'
LexError at line 5, column 1: unterminated string literal
```

### 6.2 Parse Errors

```
ParseError at line 2, column 5: expected expression
ParseError at line 1, column 5: expected identifier after 'let'
ParseError at line 4, column 3: expected '}'
```

### 6.3 Runtime Errors

```
RuntimeError: undefined variable 'x'
RuntimeError: cannot add Number and Bool
RuntimeError: cannot apply '-' to String and Number
RuntimeError: division by zero
RuntimeError: function 'add' expected 2 arguments but got 1
RuntimeError: attempted to call non-function value Number
RuntimeError: cannot return outside of a function
```

---

## 7. Grammar (EBNF)

```
program     = stmt* EOF
stmt        = fn_decl | return_stmt | let_stmt | print_stmt | if_stmt | block | expr_stmt
fn_decl     = "fn" IDENT "(" params ")" fn_body
return_stmt = "return" expr?
let_stmt    = "let" IDENT "=" expr
print_stmt  = "print" "(" expr ")"
if_stmt     = "if" expr block ("else" block)?
block       = "{" stmt* "}"
fn_body     = "{" stmt* "}"
params      = (IDENT ("," IDENT)*)?
expr_stmt   = expr

expr        = equality
equality    = comparison (("==" | "!=") comparison)*
comparison  = term (("<" | "<=" | ">" | ">=") term)*
term        = factor (("+" | "-") factor)*
factor      = unary (("*" | "/") unary)*
unary       = ("-" | "!") unary | call
call        = primary ("(" args ")")*
primary     = NUMBER | STRING | "true" | "false" | IDENT | "(" expr ")"
args        = (expr ("," expr)*)?
```
