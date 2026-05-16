# Kimin Language Specification — Milestone 8D

This document describes the syntax and semantics implemented through Milestone 8D.

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
let  mut  if  else  print  fn  return  true  false  state  transition  simulate  step
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

| Type       | Examples               | Notes |
|------------|------------------------|-------|
| Number     | `42`, `3.14`           | IEEE 754 `f64` |
| Text       | `"hello"`              | UTF-8 string |
| Bool       | `true`, `false`        | |
| Nil        | (runtime only)         | No literal syntax |
| Function   | (runtime only)         | `FunctionValue` in the interpreter |
| StateValue | `Door.closed` at runtime | Produced by state variant expressions |

### 2.2 Static types (Milestones 3–5)

The static type checker uses the same names as the runtime types. Type annotations are written as `Number`, `Text`, `Bool`, `Nil`, any unit name from the unit registry, or a state machine name.

Functions without a return type annotation are assigned `Unknown` — the gradual-typing escape hatch. Operations involving an `Unknown` value propagate `Unknown` without error, so unannotated code remains valid.

### 2.3 Unit types (Milestone 4)

A unit type is a `Number` annotated with a physical unit. Unit types are **static-only**: the runtime sees plain `f64` values; no unit information is stored at execution time.

```kimin
let distance: meters = 10   // type is meters (a NumberWithUnit)
let time: seconds = 2       // type is seconds
```

**Unit registry** — recognized unit names and their canonical forms:

| Annotation(s) | Canonical name |
|---------------|---------------|
| `m`, `meters` | `meters` |
| `s`, `seconds` | `seconds` |
| `ms`, `milliseconds` | `milliseconds` |
| `min`, `minutes` | `minutes` |
| `h`, `hours` | `hours` |
| `kg`, `kilograms` | `kilograms` |
| `A`, `amps`, `amperes` | `amperes` |
| `K`, `kelvin` | `kelvin` |
| `mol`, `moles` | `moles` |
| `cd`, `candela` | `candela` |
| `rad`, `radians` | `radians` |
| `deg`, `degrees` | `degrees` |
| `V`, `volts` | `volts` |
| `W`, `watts` | `watts` |
| `J`, `joules` | `joules` |
| `N`, `newtons` | `newtons` |

**Promotion** — a plain `Number` literal or expression satisfies a unit annotation:

```kimin
let d: meters = 10      // ok: Number literal promoted to meters
let d2: meters = d      // ok: meters assigned to meters
// let n: Number = d    // TypeError: cannot strip unit
```

**Unit arithmetic rules:**

| Operation | Result |
|-----------|--------|
| `u + u` (same unit) | `u` |
| `u + v` (different units) | TypeError |
| `u - u` (same unit) | `u` |
| `u - v` (different units) | TypeError |
| `Number * u` | `u` |
| `u * Number` | `u` |
| `u * v` (two distinct units) | `u*v` (compound unit inferred) |
| `u * u` | `u^2` (compound unit inferred) |
| `(u/v) * v` | `u` (compound simplification) |
| `u / Number` | `u` |
| `u / u` (same unit) | `Number` (dimensionless ratio) |
| `u / v` (different units) | `u/v` (compound unit inferred) |
| `Number / u` | `1/u` (reciprocal, inferred) |

**Compound unit display format:**
- Positive exponents in numerator, negative in denominator, sorted alphabetically
- Single unit: `meters`, `seconds`
- Squared: `meters^2`
- Product: `kilograms*meters`
- Quotient: `meters/seconds`
- Reciprocal: `1/seconds`
- Complex: `kilograms*meters/seconds^2`

**Compound unit annotations** — compound types can only be inferred; there is no source syntax for annotating a variable with a compound unit. Only base unit names are valid in type positions:

```kimin
let d: meters = 10
let t: seconds = 2
let speed = d / t    // type: meters/seconds (inferred)
// let v: meters/seconds = speed  // ParseError — compound annotations not allowed
```

**Unit comparison rules:**

| Operation | Result |
|-----------|--------|
| `u < u`, `u <= u`, `u > u`, `u >= u` (same unit) | `Bool` |
| `u < v` etc. (different units) | TypeError |
| `u == u`, `u != u` (same unit) | `Bool` |
| `u == v`, `u != v` (different units) | TypeError |

### 2.4 State types (Milestone 5)

A state type represents a variable that holds one variant of a declared state machine.

```kimin
state Door {
  closed
  opening
  open

  transition closed -> opening
  transition opening -> open
}

let door: Door = Door.closed
```

State types are static and runtime. At runtime, a state value prints as `StateName.variant` (e.g., `Door.closed`).

**State variant expressions** — a state value is written as `StateName.variant`:

```kimin
Door.closed
Door.opening
```

**Transition statements** — controlled state mutation. Only `transition` can mutate a state variable:

```kimin
transition door -> opening   // ok: closed -> opening is declared
transition door -> open      // ok: opening -> open is declared
transition door -> closed    // TypeError: no open -> closed transition declared
```

**Static checking rules:**

| Check | Result |
|-------|--------|
| State machine name does not exist | TypeError |
| Variant not declared in the state machine | TypeError |
| Transition declared and current variant is known statically | ok |
| Transition not declared and current variant is known | TypeError |
| Target variant does not exist | TypeError |
| Transition on a non-state variable | TypeError |
| Current variant unknown (e.g., returned from a function) | transition allowed if target variant exists |

**Known-variant tracking** — after each valid transition, the type checker updates its record of the variable's current variant. Subsequent transitions are checked against the updated variant:

```kimin
transition door -> opening   // checker now knows: door = opening
transition door -> open      // ok: opening -> open declared
transition door -> closed    // TypeError: no open -> closed transition declared
```

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

Static rule: `+` requires `Number + Number`, `Text + Text`, or `u + u` (same unit). Mixing incompatible types is a `TypeError`.

### 3.3 Comparisons

```kimin
score > 10
x == 5
name != "error"
```

All comparison operators return `Bool`.

Static rule: `<`, `<=`, `>`, `>=` require `Number` operands or same-unit operands. `==` and `!=` require both operands to be the same type (including same unit).

### 3.4 Unary Operators

```kimin
-x       // numeric negation; requires Number or unit type
!cond    // logical NOT; requires Bool
```

Static rule: `-` requires `Number` or a unit type (result preserves the unit); `!` requires `Bool`.

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

### 4.6 State Declaration

```kimin
state Name {
  variant1
  variant2
  ...

  transition variant1 -> variant2
  ...
}
```

Declares a state machine named `Name` with a set of variants and allowed transitions. State machine names are globally visible. State declarations have no runtime effect.

### 4.7 Transition Statement

```kimin
transition variable -> target_variant
```

Mutates a state variable to a new variant. State variables must use `transition` — the assignment statement (`x = expr`) does not apply to state-typed variables.

```kimin
let door: Door = Door.closed
transition door -> opening
transition door -> open
```

Static rules:
- `variable` must exist and have a state machine type.
- `target_variant` must be a declared variant of that state machine.
- If the checker has statically tracked the current variant and the transition `(current, target)` is not declared, a `TypeError` is raised.

### 4.8 Simulate Statement

```kimin
simulate <duration> step <step> {
  <stmts>
}
```

Runs a deterministic simulation loop. The body executes `floor(duration / step)` times. On each iteration, the injected variable `time` holds the elapsed time: `time = i * step` for iteration index `i` (starting at 0).

```kimin
let duration: seconds = 3
let dt: seconds = 1

simulate duration step dt {
  print(time)   // prints 0, 1, 2
}
```

Static rules:
- `duration` must be a time unit (`seconds`, `milliseconds`, `minutes`, or `hours`), or `Unknown` for gradual typing.
- `step` must have the exact same unit type as `duration` (or `Unknown`). No conversion between time units.
- Plain `Number` for duration or step is a `TypeError`.
- `time` is defined only inside the simulate body; referencing it outside is a `TypeError`.
- The body is type-checked once with `time` in scope.

Runtime rules:
- `step <= 0` → `RuntimeError`
- `duration < 0` → `RuntimeError`
- State mutations (`transition`) inside the body affect the outer variable and persist across iterations.
- `let mut` assignments inside the body update outer mutable variables via scope-chain walk.

---

### 4.9 Mutable Variables and Assignment

Variables are **immutable by default**. Reassignment requires an explicit `mut` modifier:

```kimin
let mut x: Number = 0
x = x + 1
print(x)   // 1
```

Unit-typed variables work the same way:

```kimin
let mut distance: meters = 10
let extra: meters = 5
distance = distance + extra
print(distance)   // 15
```

A plain `Number` literal or expression promotes to the target unit type at the assignment site (same rule as `let` initializers):

```kimin
let mut d: meters = 10
d = 20   // ok — Number promotes to meters
```

**Static rules:**
- `let x = ...` creates an immutable binding.
- `let mut x = ...` creates a mutable binding.
- `x = expr` reassigns `x`; `x` must be declared with `let mut`.
- The type of `expr` must equal the declared type of `x`, or be `Number` when `x` has a unit type.
- Assignment to a state-typed variable is a `TypeError` — use `transition`.
- Assignment is a statement only; it does not produce a value.
- No compound assignment operators (`+=`, `-=`, etc.).

**Error examples:**
```kimin
let x: Number = 1
x = 2   // TypeError: cannot assign to immutable variable 'x'

let mut x: Number = 1
x = "hi"   // TypeError: variable 'x' has type Number but assigned value has type Text

let mut d: meters = 1
let t: seconds = 2
d = t   // TypeError: variable 'd' has type meters but assigned value has type seconds

state Door { closed open transition closed -> open }
let mut door: Door = Door.closed
door = Door.open   // TypeError: state variables must be changed with transition, not assignment
```

**Simulate interaction:**

Mutable outer variables can accumulate values across simulate iterations:

```kimin
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
| `let x: T = e` | `e` must have type `T` (Number promotes to unit) |
| `let x = e` | type inferred from `e` |
| `fn f(p: T) -> R` | body must return `R`; args must match param types (Number promotes to unit) |
| `if cond` | `cond` must be `Bool` |
| `!x` | `x` must be `Bool` |
| `-x` | `x` must be `Number` or a unit type |
| `a + b` | both `Number`, both `Text`, or both same unit |
| `a - b` | both `Number` or both same unit |
| `a * b` | `Number * Number`, `Number * unit`, `unit * Number`, or `unit * unit` (compound inferred) |
| `a / b` | `Number / Number`, `unit / Number`, `unit / unit` (same → `Number`), `unit / unit` (different → compound), `Number / unit` (reciprocal) |
| `a < b`, `a <= b`, `a > b`, `a >= b` | both `Number` or both same unit |
| `a == b`, `a != b` | both same type (including same unit) |

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
ParseError at line 1, column 10: unknown type 'Numbr'; expected Number, Text, Bool, Nil, or a known unit (meters, seconds, kilograms, ...)
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
TypeError at line 3, column 5: cannot add meters and seconds
TypeError at line 2, column 5: variable 'bad' declared as seconds but initializer has type meters
TypeError at line 4, column 5: cannot add meters/seconds and meters
TypeError at line 4, column 5: variable 'v' declared as meters but initializer has type meters/seconds
TypeError at line 7, column 1: invalid transition for Door: closed -> closed
TypeError at line 6, column 18: unknown variant 'locked' for state machine 'Door'
TypeError at line 2, column 1: 'x' has type Number, not a state machine; transition requires a state variable
TypeError at line 1, column 7: unknown state machine 'Motor'
TypeError at line 1, column 5: duplicate variant 'closed' in state machine 'Door'
```

### 7.4 Runtime Errors

```
RuntimeError: division by zero
```

Most errors that were previously RuntimeErrors (undefined variable, wrong arity, etc.) are now TypeErrors caught before execution.

---

## 8. Grammar (EBNF)

```
program         = stmt* EOF
stmt            = state_decl | transition_stmt | simulate_stmt | fn_decl | return_stmt | let_stmt | assign_stmt
                | print_stmt | if_stmt | block | expr_stmt
state_decl      = "state" IDENT "{" (variant_decl | inner_transition)* "}"
variant_decl    = IDENT
inner_transition = "transition" IDENT "->" IDENT
transition_stmt = "transition" IDENT "->" IDENT
simulate_stmt   = "simulate" expr "step" expr "{" stmt* "}"
fn_decl         = "fn" IDENT "(" params ")" ("->" type_ann)? fn_body
return_stmt     = "return" expr?
let_stmt        = "let" "mut"? IDENT (":" type_ann)? "=" expr
assign_stmt     = IDENT "=" expr    (only when IDENT followed by single "=", not "==")
print_stmt      = "print" "(" expr ")"
if_stmt         = "if" expr block ("else" block)?
block           = "{" stmt* "}"
fn_body         = "{" stmt* "}"
params          = (typed_param ("," typed_param)*)?
typed_param     = IDENT ":" type_ann
type_ann        = "Number" | "Text" | "Bool" | "Nil" | UNIT_NAME | STATE_NAME
STATE_NAME      = IDENT (resolved to a state machine name by the type checker)
UNIT_NAME       = "m" | "meters" | "s" | "seconds"
                | "ms" | "milliseconds" | "min" | "minutes" | "h" | "hours"
                | "kg" | "kilograms"
                | "A" | "amps" | "amperes" | "K" | "kelvin"
                | "mol" | "moles" | "cd" | "candela"
                | "rad" | "radians" | "deg" | "degrees"
                | "V" | "volts" | "W" | "watts" | "J" | "joules" | "N" | "newtons"
expr_stmt       = expr

expr            = equality
equality        = comparison (("==" | "!=") comparison)*
comparison      = term (("<" | "<=" | ">" | ">=") term)*
term            = factor (("+" | "-") factor)*
factor          = unary (("*" | "/") unary)*
unary           = ("-" | "!") unary | call
call            = primary ("(" args ")")*
primary         = NUMBER | STRING | "true" | "false"
                | IDENT "." IDENT        (state variant expression)
                | IDENT                  (variable reference)
                | "(" expr ")"
args            = (expr ("," expr)*)?
```

---

## Implementation Note: Bytecode IR and VM (Milestones 8A–8D)

Language semantics are defined by the tree-walk interpreter (`kimin run`). The bytecode compiler (`kimin bytecode`) and VM (`kimin vm`) are a separate experimental execution path.

### What the bytecode VM executes (M8C + M8D)

- All core expressions: literals, arithmetic, comparisons, string concatenation, unary operators
- Variable access and mutation (globals and block-scoped locals)
- Control flow: `if`/`else`, blocks with lexical scope
- Named function declarations and calls (including recursion)
- **State machine declarations** (`state Name { ... }`) — registers name, variants, and allowed transitions in the VM state registry
- **State variant values** (`Door.closed`) — validated against the registry, pushed as `Value::StateValue { state_name, variant_name }`
- **Transition statements** (`transition door -> opening`) — validates the edge exists in the registry, updates the variable in-place (locals first, then globals)

### What remains as `UNSUPPORTED` in the VM

- Simulate blocks (`simulate`) — emit `Unsupported("simulate")` in the compiler; produce `RuntimeError: bytecode feature not yet executable: simulate` at runtime
- Computed/dynamic callees (`get_fn()(args)`) — emit `UNSUPPORTED(dynamic call)`

### Bytecode IR structures

```
BytecodeProgram {
  main: Chunk,                    // top-level instructions
  functions: Vec<FunctionChunk>,  // one per fn decl, in source order
}

FunctionChunk {
  name: String,
  params: Vec<String>,
  arity: usize,
  chunk: Chunk,
}
```

### Instructions

| Instruction | Meaning |
|-------------|---------|
| `LOAD_FUNCTION name` | Push reference to named function from function table |
| `CALL name arg_count` | Call named function; `arg_count` arguments already on stack |
| `DEFINE_STATE name variants=[...] transitions=[...]` | Register state machine metadata in VM registry; no stack effect |
| `LOAD_STATE state.variant` | Validate and push `Value::StateValue { state_name, variant_name }` |
| `TRANSITION var -> target` | Read var, validate transition edge, update var in-place |
