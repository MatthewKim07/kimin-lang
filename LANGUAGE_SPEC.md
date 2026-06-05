# Kimin Language Specification — Milestone 13B

This document describes the syntax and semantics implemented through Milestone 13B.

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
let  mut  if  else  while  for  in  break  continue  print  fn  return  true  false  state  transition  simulate  step
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
| `=`   | Assignment (in `let` and `x = expr`) |
| `+=`  | Compound add-assign |
| `-=`  | Compound subtract-assign |
| `*=`  | Compound multiply-assign |
| `/=`  | Compound divide-assign |
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

The static type checker uses the same names as the runtime types. Type annotations are written as `Number`, `Text`, `Bool`, `Nil`, `Array<T>`, `Map<Text, V>`, any unit name from the unit registry, or a state machine name.

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

### 4.9 While Loop

```kimin
while <Bool-expr> {
  <stmts>
}
```

Repeats the body as long as the condition evaluates to `true`. The condition is re-evaluated before every iteration.

```kimin
let mut x = 0
while x < 5 {
    print(x)
    x += 1
}
// prints 0, 1, 2, 3, 4
```

Static rules:
- The condition must have type `Bool`. Any other type (including unit types and state types) is a `TypeError`.
- Variables declared inside the body are scoped to each iteration and do not leak to the enclosing scope.
- The body may contain any statement including `if`, nested `while`, `simulate`, function calls, and compound assignment.
- Immutability rules apply inside while bodies: assigning to an immutable variable is a `TypeError`.

Runtime rules:
- If the condition evaluates to a non-`Bool` value, a `RuntimeError` is raised.
- Each iteration runs the body inside a fresh child scope; mutations to outer `let mut` bindings persist across iterations.
- `return` inside a while body propagates out to the enclosing function.
- State `transition` statements inside the body affect the outer state variable and persist across iterations.

Bytecode:
```
@loop_start: <condition>
             JUMP_IF_FALSE @loop_end
             BEGIN_SCOPE
             <body>
             END_SCOPE
             JUMP @loop_start
@loop_end:
```

No new VM instructions are required — `Jump` and `JumpIfFalse` already handle the loop structure.

---

### 4.10 Break and Continue

`break` and `continue` are statement-only forms that control while-loop iteration.

```kimin
while <Bool-expr> {
  ...
  break     // exits the nearest enclosing while loop
  continue  // skips the rest of the body and re-evaluates the condition
  ...
}
```

Example:

```kimin
let mut x: Number = 0
while x < 10 {
    x += 1
    if x == 3 { continue }
    if x == 8 { break }
    print(x)
}
// prints: 1 2 4 5 6 7
```

Static rules:
- `break` is valid only inside a `while` loop body. Using `break` outside any while → `TypeError`.
- `continue` is valid only inside a `while` loop body. Using `continue` outside any while → `TypeError`.
- Both `break` and `continue` target the **nearest** enclosing while loop. There are no labels.
- `break`/`continue` do not cross function boundaries: a function body resets the loop context, so `break` inside a function but outside any while in that function is a `TypeError`, even if the function is called from inside a while.
- `break`/`continue` do not cross simulate boundaries: `break` directly inside a simulate body (not inside an inner while) is a `TypeError`, even if the simulate is inside an outer while.
- Neither `break` nor `continue` takes a value or expression.

Runtime rules:
- `break`: exits the nearest while loop and resumes execution after the loop.
- `continue`: skips the remaining statements in the current loop body and immediately re-evaluates the while condition for the next iteration.
- `return` inside a while body still propagates out to the enclosing function, regardless of any `break`/`continue` in the same body.

Bytecode lowering:
- Both `break` and `continue` emit `EndScope` instructions to unwind all scopes opened inside the current loop body (including any nested if/block scopes), then emit a `Jump` instruction.
- `break` jumps to the instruction after the loop's `EndScope`+`Jump` (the loop exit point).
- `continue` jumps back to the start of the condition evaluation (before `JumpIfFalse`).
- No new VM instructions are required.

---

### 4.11 For/Range Loops

A `for` loop iterates a variable over a numeric range:

```kimin
for i in range(start, end) {
    <body>
}
```

The loop variable `i` takes integer values from `start` (inclusive) to `end` (exclusive), incrementing by 1 each iteration.

```kimin
// Prints 0 1 2 3 4
for i in range(0, 5) {
    print(i)
}

// Sum 1 through 10
let mut total: Number = 0
for i in range(1, 11) {
    total += i
}
print(total)   // 55
```

**Static rules:**
- `start` and `end` must have type `Number`. Providing a unit type → `TypeError`.
- `range` takes exactly 2 arguments. 3 or more arguments → `ParseError`.
- The loop variable is immutable. Assigning to it inside the body → `TypeError`.
- The loop variable is loop-local. It is not visible after the loop body.
- `break` and `continue` are valid inside a `for` loop body (same rules as `while`).
- `break`/`continue` do not cross function or simulate boundaries.

**Runtime rules:**
- If `start >= end`, the body executes zero times.
- The loop variable is incremented by exactly 1.0 after each iteration.
- Mutations to outer mutable variables persist across iterations.
- `break` exits the loop immediately.
- `continue` skips the rest of the current iteration and jumps to the increment step (not the condition re-check — the variable is incremented before the condition is re-evaluated).
- `return` inside a for body propagates out to the enclosing function.

**Error examples:**
```kimin
let t: seconds = 5
for i in range(0, t) { }   // TypeError: range end must be Number, got seconds

for i in range(0, 3) { i = 1 }   // TypeError: cannot assign to immutable variable 'i'

for i in range(0, 3) { }
print(i)   // TypeError: undefined variable 'i'

for i in range(0, 5, 1) { }   // ParseError: range takes exactly 2 arguments
```

**Bytecode lowering:**

The compiler emits:
1. `BeginScope` (outer) — holds loop var `i` and a hidden sentinel `__kimin_range_end_N`
2. `start` expression → `DefineLocal i`
3. `end` expression → `DefineLocal __kimin_range_end_N`
4. `@loop_start:` `LoadLocal i`, `LoadLocal __kimin_range_end_N`, `LESS`, `JumpIfFalse(@loop_end)`
5. `BeginScope` (body) — body statements
6. `EndScope` (body)
7. `@increment:` `LoadLocal i`, `CONSTANT 1`, `ADD`, `StoreLocal i`, `Jump(@loop_start)`
8. `@loop_end:` `EndScope` (outer)

`break` jumps to `@loop_end`; `continue` jumps to `@increment`. No new VM instructions.

---

### 4.12 For-Each Loops

A for-each loop iterates over every element of an array expression:

```kimin
for item in array_expr {
    <body>
}
```

The loop variable `item` takes the value of each element in order, from index 0 to the last.

```kimin
let scores = [85, 92, 78, 95]
let mut total: Number = 0
for score in scores {
    total += score
}
print(total)   // 350

// Works with any Array<T>
for word in split("hello world kimin", " ") {
    print(len(word))
}

// Iterate over map keys or values
let m = {"a": 1, "b": 2, "c": 3}
let mut s: Number = 0
for v in values(m) {
    s += v
}
print(s)   // 6
```

**Static rules:**
- `array_expr` must have type `Array<T>`. Any other type → `TypeError: for-each requires Array, got ...`.
- The loop variable type is `T` (the element type of `Array<T>`).
- The loop variable is immutable. Assigning to it inside the body → `TypeError`.
- The loop variable is loop-local. It is not visible after the loop body.
- `break` and `continue` are valid inside a for-each body (same rules as while/for-range).
- `break`/`continue` do not cross function or simulate boundaries.

**Runtime rules:**
- `array_expr` is evaluated exactly once before the first iteration (snapshot semantics).
- Mutations to the source array inside the body (e.g. via `push`) do not affect the iteration count or element order — the loop iterates over the snapshot.
- Empty array → zero iterations; no error.
- `break` exits the loop immediately.
- `continue` skips the rest of the current iteration and jumps to the index-increment step.
- `return` inside a for-each body propagates out to the enclosing function.
- Mutations to outer mutable variables persist across iterations.

**Error examples:**
```kimin
for x in 42 { }             // TypeError: for-each requires Array, got Number
for x in "hello" { }        // TypeError: for-each requires Array, got Text
for x in [1, 2] { x = 99 } // TypeError: cannot assign to immutable variable 'x'
for x in [1, 2] { }
print(x)                    // TypeError: undefined variable 'x'
```

**Bytecode lowering:**

The compiler emits (no new bytecode instructions):
1. `BeginScope` (outer) — holds hidden `__kimin_foreach_iter_N` (array snapshot) and `__kimin_foreach_idx_N` (index counter)
2. `array_expr` → `DefineLocal __kimin_foreach_iter_N`
3. `CONSTANT 0` → `DefineLocal __kimin_foreach_idx_N`
4. `@loop_start:` `LoadLocal idx`, `LoadLocal iter`, `LEN`, `LESS`, `JumpIfFalse(@loop_end)`
5. `BeginScope` (body) — `LoadLocal iter`, `LoadLocal idx`, `INDEX`, `DefineLocal item` — then body statements
6. `EndScope` (body)
7. `@increment:` `LoadLocal idx`, `CONSTANT 1`, `ADD`, `StoreLocal idx`, `Jump(@loop_start)`
8. `@loop_end:` `EndScope` (outer)

`break` jumps to `@loop_end`; `continue` jumps to `@increment`. No new VM instructions.

---

### 4.12.1 Indexed For-Each Loops

An **indexed for-each loop** provides the 0-based index alongside each element:

```kimin
for i, item in array_expr {
    // i: immutable Number (0-based index)
    // item: immutable T (element of Array<T>)
    print(i)
    print(item)
}
```

**Syntax:**
```
for_each_indexed_stmt = "for" IDENT "," IDENT "in" expr "{" stmt* "}"
```
The first identifier becomes the index variable (`Number`); the second becomes the element variable (`T`).

**Type rules:**
- `array_expr` must be `Array<T>`. Any other type → `TypeError: for-each requires Array, got ...`.
- The two variable names must be distinct: `for x, x in arr` → `TypeError: indexed for-each variable names must be distinct`.
- Both variables are immutable and loop-local; assigning to either is a `TypeError`.
- Both variables are out of scope after the loop body closes.
- `break` and `continue` are valid inside an indexed for-each body.
- `return` inside an indexed for-each body propagates out to the enclosing function.

**Semantics:**
- `array_expr` is evaluated once before iteration begins (snapshot semantics, identical to M13A).
- On iteration `k` (0-based), `i = k` (Number) and `item` = the k-th element.
- Empty array → zero iterations, no error.
- Mutations to the source array inside the body do not affect iteration count (snapshot).

**Examples:**

```kimin
let nums = [10, 20, 30]
for i, n in nums {
    print(i)    // 0, 1, 2
    print(n)    // 10, 20, 30
}

// Use index to write into a target array
let src = [1, 2, 3]
let mut dst = [0, 0, 0]
for i, v in src {
    dst[i] = v * 2
}

// Find first occurrence
fn find(arr: Array<Number>, target: Number) -> Number {
    for i, v in arr {
        if v == target { return i }
    }
    return -1
}
```

**Error examples:**
```kimin
for i, v in 42 { }             // TypeError: for-each requires Array, got Number
for x, x in [1, 2] { }        // TypeError: indexed for-each variable names must be distinct
```

**Bytecode lowering:**

Same hidden-sentinel layout as M13A for-each, with one addition: `LOAD_LOCAL __kimin_foreach_idx_N → DEFINE_LOCAL index_name` in the body scope before the element definition.

```
BEGIN_SCOPE (outer — __kimin_foreach_iter_N, __kimin_foreach_idx_N)
  <iterable> → DEFINE_LOCAL __kimin_foreach_iter_N
  CONSTANT 0 → DEFINE_LOCAL __kimin_foreach_idx_N
@loop_start:
  LOAD_LOCAL idx  LEN  LESS  JUMP_IF_FALSE @loop_end
  BEGIN_SCOPE (body — index_name, var_name)
    LOAD_LOCAL __kimin_foreach_idx_N → DEFINE_LOCAL index_name
    LOAD_LOCAL __kimin_foreach_iter_N  LOAD_LOCAL __kimin_foreach_idx_N  INDEX → DEFINE_LOCAL var_name
    <body>
  END_SCOPE (body)
@increment:
  LOAD_LOCAL idx  CONSTANT 1  ADD  STORE_LOCAL idx  JUMP @loop_start
@loop_end:
END_SCOPE (outer)
```

No new VM instructions.

---

### 4.13 Arrays

A **fixed-size typed array** is a homogeneous sequence of values.

#### Array type annotations

Array types can be written explicitly as `Array<T>` where `T` is one of:

- `Number`, `Text`, `Bool`, `Nil`
- A unit name: `meters`, `seconds`, `kilograms`, …
- A state machine name: `Door`, `TrafficLight`, …

Nested arrays (`Array<Array<T>>`) are a `ParseError`.

```kimin
let nums: Array<Number> = [1, 2, 3]
let words: Array<Text> = ["a", "b"]
let flags: Array<Bool> = [true, false]

fn sum(nums: Array<Number>) -> Number { ... }
fn make() -> Array<Number> { return [1, 2, 3] }
```

#### Array literals

```kimin
let nums = [10, 20, 30]
let words = ["hello", "world"]
let flags = [true, false, true]
```

- Trailing commas are allowed: `[1, 2,]` is valid.
- All elements must have the same type. Mixed types are a `TypeError`:

```kimin
[1, "two"]   // TypeError: array elements must have the same type
```

#### Empty array literals

An empty array literal `[]` is allowed only when the expected element type is known from context:

```kimin
let nums: Array<Number> = []          // OK — annotation provides type
fn make() -> Array<Number> { return [] }  // OK — return type provides type

let nums = []   // TypeError: element type cannot be inferred
```

The expected-type context is:
- `let x: Array<T> = []` — annotation provides `T`
- `return []` inside a function with `-> Array<T>` return type
- `x = []` where `x` is already typed as `Array<T>`
- `f([])` where the corresponding parameter is typed `Array<T>` — call argument position

#### Call argument expected-type propagation

When calling a statically known function, each argument is typechecked with its corresponding parameter type as the expected type. This means empty array literals are accepted in call arguments when the parameter type is `Array<T>`:

```kimin
fn sum(nums: Array<Number>) -> Number {
  let mut total: Number = 0
  for i in range(0, len(nums)) {
    total += nums[i]
  }
  return total
}

print(sum([]))        // 0 — [] is Array<Number> because param is Array<Number>
print(sum([1, 2, 3])) // 6
```

The rule applies to all `Array<T>` parameter types including unit arrays and state arrays. Dynamic or unknown callees do not provide expected types for arguments.

#### Indexing

```kimin
let arr = [10, 20, 30]
print(arr[0])    // 10
print(arr[2])    // 30
```

- The index expression must be a `Number`. A non-Number index is a `TypeError`.
- Runtime checks:
  - Index must be an integer (fractional → `RuntimeError`).
  - Index must be ≥ 0 (negative → `RuntimeError`).
  - Index must be less than `len(arr)` (out-of-bounds → `RuntimeError`).

#### Slicing

```kimin
let nums = [10, 20, 30, 40]
let middle = nums[1..3]
print(middle[0])   // 20
print(middle[1])   // 30
print(len(middle)) // 2
```

- Slice syntax is `array_expr[start_expr..end_expr]`.
- The target expression must have type `Array<T>`.
- `start_expr` and `end_expr` must have type `Number`.
- The result type is `Array<T>`.
- Slices are end-exclusive: `start` is included and `end` is not.
- Runtime checks:
  - Start and end must be integer-like numbers.
  - Start and end must be non-negative.
  - Start must be less than or equal to end.
  - End must be less than or equal to `len(array)`.
- Slices create a new independent array value. Mutating the original does not mutate the slice, and mutating a mutable slice binding does not mutate the original.
- Slices may produce empty arrays at runtime, e.g. `arr[2..2]`.
- Slice assignment, slice compound assignment, open-ended slices, and step slices are unsupported.

#### `len` builtin

```kimin
let arr = [1, 2, 3]
print(len(arr))   // 3
```

- `len` takes exactly one argument of type `Array<T>` or `Text`.
- Returns a `Number`.
- A non-Array, non-Text argument is a `TypeError`.

#### Using arrays with loops

```kimin
let primes = [2, 3, 5, 7, 11]
let mut sum = 0
for i in range(0, len(primes)) {
    sum += primes[i]
}
print(sum)   // 28
```

#### Index assignment

```kimin
let mut nums = [10, 20, 30]
nums[1] = 99
print(nums[1])   // 99
```

- Index assignment is a **statement**, not an expression.
- The target must be a mutable array binding declared with `let mut`.
- The index expression must type-check as `Number`.
- The assigned value must match the array element type.
- A plain `Number` may still promote to a unit-typed array element slot, matching normal assignment rules.
- Runtime checks match index reads:
  - Index must be an integer.
  - Index must be non-negative.
  - Index must be less than `len(arr)`.
- The array remains fixed-size; only element replacement is supported.

#### Restrictions

- **No nested arrays**: nested arrays are documented as unsupported.
- **`len` is a builtin** — also accepts `Text` since M11A.
- **`len` is a builtin**, not a user-defined function. A user-defined function named `len` with a single array argument would be shadowed by the builtin.
#### Index compound assignment

```kimin
let mut nums = [10, 20, 30]
nums[0] += 5
nums[1] *= 2
print(nums[0])   // 15
print(nums[1])   // 40
```

- Index compound assignment is a **statement**, not an expression.
- Supported operators: `+=`, `-=`, `*=`, `/=`.
- The target must be a mutable array binding declared with `let mut`.
- The index expression must type-check as `Number`.
- The rhs must be valid with the current element type under the same rules as normal compound assignment.
- The index is evaluated **once**, then the current element is read, combined with the rhs, and written back.
- Runtime checks match index reads:
  - Index must be an integer.
  - Index must be non-negative.
  - Index must be less than `len(arr)`.

#### `push` and `pop` builtins

```kimin
let mut log = [0]
push(log, 1)
push(log, 2)
print(len(log))   // 3

let v = pop(log)
print(v)          // 2
print(len(log))   // 2
```

- `push(arr, value)` — appends `value` to the end of `arr`; returns `Nil`.
- `pop(arr)` — removes and returns the last element of `arr`; `RuntimeError` if the array is empty.
- Both require a **mutable array variable** as the first argument (plain identifier only; literal expressions rejected at the typechecker).
- `push` checks that `value` is type-compatible with the element type (same rules as `IndexAssign`, including Number→unit promotion).
- `pop` return type is the element type `T` of `Array<T>`.
- `push` and `pop` are **builtins**, not user-defined functions; they take precedence over any user-defined function with those names.

#### Bytecode lowering

| Operation | Instructions emitted |
|-----------|---------------------|
| `[e1, e2, e3]` | compile e1, e2, e3 left-to-right; emit `ARRAY 3` |
| `arr[i]` | compile arr; compile i; emit `INDEX` |
| `arr[start..end]` | compile arr; compile start; compile end; emit `SLICE` |
| `len(arr)` | compile arr; emit `LEN` (no `CALL` instruction) |
| `arr[i] = value` | compile i; compile value; emit `SET_INDEX name` |
| `arr[i] op= value` | compile i; compile value; emit `INDEX_COMPOUND_ASSIGN name op` |
| `push(arr, value)` | compile value; emit `ARRAY_PUSH name` (no `CALL` instruction) |
| `pop(arr)` | emit `ARRAY_POP name` (no `CALL` instruction; pushes element) |
| `len(s)` (Text) | compile s; emit `LEN` |
| `s[i]` (Text) | compile s; compile i; emit `INDEX` |
| `s[start..end]` (Text) | compile s; compile start; compile end; emit `SLICE` |

---

### 4.13 String Indexing and Slicing

Strings support three read-only operations using the same syntax as arrays.

```kimin
let s = "hello"
print(len(s))     // 5
print(s[0])       // h
print(s[1..4])    // ell
```

#### `len(s)`

- Accepts a `Text` argument.
- Returns a `Number` equal to the number of Unicode scalar values (Rust `char`s).
- Reuses the same `LEN` bytecode instruction as `len(arr)`.

#### `s[i]` — character index

- The target must be `Text`; the index must be `Number`.
- Returns a `Text` value containing exactly one character.
- A non-Number index is a `TypeError`.
- Runtime checks:
  - Index must be integer-like (no fractional part).
  - Index must be non-negative.
  - Index must be less than `len(s)` (out-of-bounds → `RuntimeError`).

#### `s[start..end]` — substring slice

- The target must be `Text`; `start` and `end` must be `Number`.
- Returns a `Text` substring, char-indexed, end-exclusive.
- Runtime checks:
  - Start and end must be integer-like and non-negative.
  - `start ≤ end`.
  - `end ≤ len(s)`.

#### Unicode policy

Indexing operates on Unicode scalar values (Rust `char`s). This means:
- ASCII characters (`a`–`z`, `0`–`9`, etc.) each count as one index unit.
- Multi-byte codepoints like `é`, `中`, `🎵` each count as one index unit.
- Grapheme clusters composed of multiple codepoints (e.g., emoji sequences using zero-width joiners) are **not** handled as a unit; each codepoint is a separate index unit.

#### Restrictions

- **No string mutation**: `s[i] = "x"`, `push(s, "a")`, and `pop(s)` are all unsupported (`TypeError`).
- **No open-ended or stepped slices**: `s[1..]`, `s[..3]`, and `s[1..5..2]` are unsupported.
- **No char type**: single-character results are `Text` values, not a separate `Char` type.

---

### 4.14 String Utility Builtins

Three read-only string predicates are built into the language.

```kimin
print(contains("hello world", "world"))   // true
print(contains("hello world", "x"))       // false
print(starts_with("hello", "he"))         // true
print(starts_with("hello", "lo"))         // false
print(ends_with("hello", "lo"))           // true
print(ends_with("hello", "he"))           // false
```

#### `contains(text, pattern) -> Bool`

Returns `true` if `pattern` appears anywhere in `text`. An empty pattern always returns `true`.

#### `starts_with(text, prefix) -> Bool`

Returns `true` if `text` begins with `prefix`. An empty prefix always returns `true`.

#### `ends_with(text, suffix) -> Bool`

Returns `true` if `text` ends with `suffix`. An empty suffix always returns `true`.

#### Type rules

- Both arguments must be `Text`.
- Return type is `Bool`.
- Wrong arity or a non-`Text` argument is a `TypeError`.

#### Implementation note

These builtins are intercepted before normal function dispatch (same pattern as `len`/`push`/`pop`). No `CALL` bytecode instruction is emitted; instead three dedicated instructions (`CONTAINS`, `STARTS_WITH`, `ENDS_WITH`) are used. Unicode correctness is inherited from Rust's `str` methods, which operate on UTF-8 bytes but still produce correct results for well-formed Unicode strings.

#### Restrictions

- **No mutation**: these builtins are read-only predicates.
- **No regex**: pattern arguments are treated as literal substrings.
- **No case-insensitive variants**: matching is always case-sensitive.

---

### 4.15 String Transformation Builtins

Three string transformation functions are built into the language.

```kimin
print(to_upper("hello"))          // HELLO
print(to_lower("HELLO"))          // hello
print(trim("  hello  "))          // hello
print(to_upper(trim("  hi  ")))   // HI
```

#### `to_upper(text) -> Text`

Returns the uppercased version of `text`. Delegates to Rust `String::to_uppercase`. String length may increase for some Unicode codepoints (e.g., German `ß` → `SS`).

#### `to_lower(text) -> Text`

Returns the lowercased version of `text`. Delegates to Rust `String::to_lowercase`.

#### `trim(text) -> Text`

Returns `text` with leading and trailing Unicode whitespace removed. Delegates to Rust `str::trim`. Does not mutate the original string.

#### Type rules

- Exactly 1 argument, which must be `Text`.
- Return type is `Text`.
- Wrong arity or a non-`Text` argument is a `TypeError`.

#### Unicode behavior

Case conversion follows Rust's Unicode rules. Results are correct for Latin, Greek, Cyrillic, and most Unicode scripts. String length may differ between input and output for certain codepoints.

#### Composition with other string builtins

Transformation builtins can be freely composed with utility builtins:

```kimin
contains(to_lower("HELLO WORLD"), "world")    // true
starts_with(trim("  hello"), "hello")         // true
len(trim("  hi  "))                           // 2
to_upper(trim("  hello  "))[0]               // H
```

#### Restrictions

- **No mutation**: transformation returns a new `Text` value; the original is unchanged.
- **No regex**: these are not pattern-based transformations.
- **No replace or case-insensitive comparison**: not yet supported.
- **Escape sequences**: Kimin string literals do not support `\t` or `\n` — trim is only observable with space characters in literal strings.

---

### 4.16 String Split Builtin

`split` splits a `Text` value into an `Array<Text>` of parts.

```kimin
let parts = split("a,b,c", ",")
print(len(parts))         // 3
print(parts[0])           // a
print(parts[1])           // b

let chars = split("abc", "")
print(len(chars))         // 3
print(chars[0])           // a
```

#### `split(text, delimiter) -> Array<Text>`

- `text` — the `Text` to split.
- `delimiter` — the `Text` separator.
- Returns a new `Array<Text>` containing the parts.

If `delimiter` is the empty string `""`, the result contains each Unicode character of `text` as a separate one-character `Text` element. Splitting an empty string with an empty delimiter returns an empty array.

If `delimiter` is non-empty, the result is equivalent to Rust `str::split(delimiter)`. Consecutive delimiters produce empty-string elements. If the delimiter does not appear in `text`, the result is a one-element array containing `text`.

#### Type rules

- Exactly 2 arguments, both must be `Text`.
- Return type is `Array<Text>`.
- Wrong arity or a non-`Text` argument is a `TypeError`.

#### Composition

`split` results are plain `Array<Text>` values. All array operations apply: `len`, `[]` indexing, `[start..end]` slices, `push`, `pop`, and iteration with `for`.

```kimin
let words = split("hello world", " ")
print(to_upper(words[0]))                   // HELLO
print(contains(words[1], "orld"))           // true
print(len(split("a,b,c", ",")))             // 3
```

#### Restrictions

- **No mutation**: `split` returns a new array; the original string is unchanged.
- **No regex delimiter**: `delimiter` is a literal string, not a pattern.
- **No limit parameter**: all occurrences are split.
- **No open-ended delimiters**: the delimiter must be an exact `Text` value.

---

### 4.11B String Join Builtin

`join` joins an `Array<Text>` into a single `Text` value with a delimiter between each element.

```kimin
let words: Array<Text> = ["hello", "world", "kimin"]
print(join(words, ", "))          // hello, world, kimin
print(join(["a", "b", "c"], "")) // abc
let empty: Array<Text> = []
print(join(empty, ","))           // (empty string)
```

#### `join(parts, delimiter) -> Text`

- `parts` — an `Array<Text>`. Required; any other type is a `TypeError`.
- `delimiter` — a `Text` value inserted between consecutive elements.

If `parts` is empty, the result is `""`. If `parts` has one element, the result is that element (delimiter is not used). Otherwise, elements are concatenated with `delimiter` between each adjacent pair — equivalent to Rust `Vec<String>.join(delimiter)`.

#### Interactions with other builtins

`join` and `split` are inverse operations when the delimiter is the same:

```kimin
let original = "a-b-c"
let parts = split(original, "-")
print(join(parts, "-"))           // a-b-c
```

#### Restrictions

- **First arg must be `Array<Text>`**: `Array<Number>`, plain `Text`, and other types are `TypeError`.
- **No generic join**: `join` works only on `Array<Text>`, not `Array<Number>` or other element types.
- **No separator-less form**: exactly 2 arguments required.
- **No regex delimiter**: `delimiter` is a literal string, not a pattern.

---

### 4.13 Maps (Milestone 12A)

A **map** is a collection of key-value pairs with `Text` keys and homogeneous values.

#### Map literals

```kimin
let scores = {"alice": 10, "bob": 20, "carol": 15}
```

- Keys must be `Text` expressions. Non-Text keys are a `TypeError`.
- All values must be the same type (homogeneous). Mixed types are a `TypeError`.
- Empty map literals `{}` require an explicit `Map<Text, V>` type annotation on the `let` binding; without one they are a `TypeError`.
- Duplicate keys: the last value in source order wins.

#### Map indexing

```kimin
print(scores["alice"])   // 10
```

- Key must be a `Text` expression. A `Number` key on a map is a `TypeError`.
- If the key is not present at runtime, a `RuntimeError` is raised: `"map key 'k' not found"`.
- `Instruction::Index` is reused; the VM and interpreter dispatch on the target value type.

#### Display format

Maps print as `{key: value, key: value, ...}` in alphabetical key order (BTreeMap ordering).

```kimin
print({"b": 2, "a": 1})   // {a: 1, b: 2}
```

#### Type

`Type::Map(Box<Type::Text>, Box<T>)` — first argument is always `Type::Text` in M12A.

#### Map mutation by key (Milestone 12B)

Mutable maps support index assignment:

```kimin
let mut scores = {"alice": 10}
scores["alice"] = 20
scores["bob"] = 5
```

- The map binding must be declared with `let mut`.
- The key expression must have type `Text`.
- The assigned value must match the map's value type.
- Assigning to an existing key replaces its value.
- Assigning to a missing key inserts a new entry.
- `Stmt::IndexAssign` is reused; the interpreter and VM dispatch on `Array` vs `Map` at runtime.

#### Map index compound assignment (Milestone 12C)

Mutable maps also support compound assignment on existing keys:

```kimin
let mut counts = {"a": 0, "b": 5}
counts["a"] += 1
counts["a"] += 2
counts["b"] *= 2
```

- Supported operators: `+=`, `-=`, `*=`, `/=`.
- The map binding must be declared with `let mut`.
- The key expression must have type `Text`.
- The right-hand side participates in the same operator rules as ordinary compound assignment.
- The key must already exist at runtime. Missing-key map compound assignment raises `RuntimeError`.
- `Stmt::IndexCompoundAssign` is reused; the interpreter and VM dispatch on `Array` vs `Map` at runtime.

#### Map builtins (Milestones 12D–12F)

Four built-in functions operate on maps:

**`has_key(map, key) -> Bool`**

Returns `true` if `key` exists in `map`, `false` otherwise.

```kimin
let scores = {"alice": 10, "bob": 20}
print(has_key(scores, "alice"))   // true
print(has_key(scores, "carol"))   // false
```

- First argument must be `Map<Text, V>`; second must be `Text`.
- Wrong arity or wrong argument type is a `TypeError` (static) and `RuntimeError` (runtime).
- Missing key returns `false` — never a `RuntimeError`.
- Useful as a guard before compound assignment or `remove` on potentially absent keys.

**`keys(map) -> Array<Text>`**

Returns all keys of `map` as an `Array<Text>` in lexicographic (alphabetical) order.

```kimin
let scores = {"bob": 20, "alice": 10}
let names = keys(scores)
print(join(names, ","))   // alice,bob
```

- Argument must be `Map<Text, V>`; result is always `Array<Text>`.
- Key order is deterministic: BTreeMap lexicographic order (same order as map display).
- Wrong arity or wrong argument type is a `TypeError` (static) and `RuntimeError` (runtime).
- Result array can be used with `len`, `join`, array indexing, and `for` loops.

**`values(map) -> Array<V>`**

Returns all values of `map` as an `Array<V>` in the same deterministic sorted-key order as `keys(map)`.

```kimin
let scores = {"bob": 20, "alice": 10}
let vals = values(scores)
print(vals[0])   // 10  (alice's score — "alice" sorts before "bob")
print(vals[1])   // 20  (bob's score)
```

- Argument must be `Map<Text, V>`; result type is `Array<V>` where `V` is the map's value type.
- Value order matches BTreeMap iteration order — same as `keys(map)` order.
- Wrong arity or wrong argument type is a `TypeError` (static) and `RuntimeError` (runtime).
- Does not mutate the map.
- Result array can be used with `len`, array indexing, and `for` loops; Text-value maps can use `join`.

**`remove(map, key) -> V`**

Removes the entry at `key` from `map` and returns the removed value. The map must be declared `let mut`.

```kimin
let mut scores = {"alice": 10, "bob": 20}
let v = remove(scores, "bob")
print(v)                          // 20
print(has_key(scores, "bob"))     // false
```

- First argument must be a plain mutable map variable identifier — expressions like `remove({...}, "k")` are a `TypeError`.
- First argument variable must be `let mut`; immutable map → `TypeError` with message `"cannot remove from immutable map 'name'"`.
- Second argument must be `Text`; wrong type is a `TypeError`.
- Return type is `V`, the map's value type.
- Missing key at runtime → `RuntimeError` with message `"map key 'k' not found"`. Use `has_key` to guard.
- After `remove`, `has_key(map, key)` returns `false` and `len(keys(map))` decreases by one.
- Unlike `m["k"] = v` (plain assignment), `remove` cannot reinsert a missing key — it only removes existing entries.

Use `keys` + `remove` to drain a map:

```kimin
let mut scores = {"alice": 10, "bob": 20}
let ks = keys(scores)
let mut total: Number = 0

for i in range(0, len(ks)) {
  total += remove(scores, ks[i])
}

print(total)   // 30
```

All four builtins are intercepted before normal function dispatch — no `CALL` instruction is emitted. Bytecode: `HAS_KEY` pops key then map, pushes `Bool`; `KEYS` pops map, pushes `Array<Text>`; `VALUES` pops map, pushes `Array<V>`; `REMOVE_KEY name` pops key, loads map `name` from env, removes, pushes removed value.

#### Map type annotations (Milestone 14A)

`Map<Text, V>` can be written as an explicit type annotation on `let` and `let mut` bindings, function parameters, and function return types. `V` can be `Number`, `Text`, `Bool`, `Nil`, any unit name, a state machine name, or `Array<T>`. Nested map annotations (`Map<Text, Map<Text, V>>`) are a `ParseError`. Non-Text key annotations (`Map<Number, V>`) are a `TypeError`.

```kimin
let mut scores: Map<Text, Number> = {}   // empty map — annotation required
scores["alice"] = 10
scores["bob"] = 20

fn total(m: Map<Text, Number>) -> Number {
    let mut sum = 0
    for v in values(m) { sum += v }
    return sum
}

fn make_empty() -> Map<Text, Bool> {
    return {}
}
```

An empty map literal `{}` is accepted when the inferred expected type (from annotation, return type, or function parameter) is `Map<Text, V>`. Without that context, `{}` remains a `TypeError`.

#### Restrictions

- **No nested maps**: `{"outer": {"inner": 1}}` is a `TypeError`; `Map<Text, Map<Text, V>>` annotation is a `ParseError`.
- **No non-Text keys**: `{1: "a"}` is a `TypeError`; `Map<Number, V>` annotation is a `TypeError`.
- **No direct map iteration**: maps cannot be iterated directly with `for`; use `for k in keys(m)` or `for v in values(m)`.
- **Empty map without annotation is TypeError**: use `let m: Map<Text, V> = {}`.
- **`remove` missing key is RuntimeError**: use `has_key` to guard before `remove` if key existence is uncertain.

---

### 4.11b Structs (Milestone 15A)

Structs are named record types with a fixed set of typed fields. They are value types — fully immutable and copied on assignment.

#### Declaration

```kimin
struct Point { x: Number, y: Number }
struct Config { name: Text, timeout: Number, enabled: Bool }
```

- `struct` keyword followed by name, then `{` field: Type, ... `}`
- At least one field required
- Field types can be `Number`, `Text`, `Bool`, any unit name, a state machine name, or `Array<T>`
- No duplicate field names; no duplicate struct names in the same program
- No nested struct types as field types (yet)

#### Construction

```kimin
let p = Point { x: 3, y: 4 }
let cfg = Config { name: "worker", timeout: 30, enabled: true }
```

- All declared fields must be provided — no omissions, no extras, no duplicates
- Fields may appear in any order in the literal
- Type of the expression is `Struct("Point")`
- No empty struct literals (`User {}` is a `ParseError`; at least 1 field required)

#### Field access

```kimin
print(p.x)        // 3
print(cfg.name)   // worker
```

Dot notation reads a named field. Field access on a non-struct type is a `TypeError`. Unknown field name is a `RuntimeError`.

#### Structs in arrays

```kimin
let points = [p1, p2, p3]
print(points[0].x)   // first element, field x
```

Chained access (`arr[i].field`, `f().field`) uses `Expr::FieldAccess` in the AST.

#### Structs in functions

```kimin
fn get_x(p: Point) -> Number { return p.x }
fn make_pt(a: Number, b: Number) -> Point { return Point { x: a, y: b } }
```

Structs can be passed as function parameters (using the struct name as the type annotation) and returned from functions.

#### Type rules

| Construct | Rule |
|-----------|------|
| `struct S { f: T }` | Declares struct type `S`; field `f` has type `T` |
| `S { f: e }` | `e` must have type `T`; result is `Type::Struct("S")` |
| `s.f` | `s` must be `Type::Struct("S")`; result is declared type of field `f` |
| `fn g(s: S)` | `S` used as parameter type annotation resolves to `Type::Struct("S")` |
| `-> S` | Return type annotation `S` resolves to `Type::Struct("S")` |

#### Bytecode

- `Instruction::StructLiteral { name, fields: Vec<String> }` — compiler emits field values left-to-right in source order; VM pops N values (LIFO), reverses to restore source order, maps to field names, builds `BTreeMap`; disassembled as `STRUCT_LITERAL name fields=[f1, f2]`
- `Instruction::FieldAccess(String)` — compiler emits object expression then instruction; VM pops struct value, looks up field by name; disassembled as `FIELD_ACCESS field_name`
- Simple `u.field` access on a variable is compiled as `LoadGlobal/Local u` + `FieldAccess("field")` (not `LoadState`)
- `state_types: HashSet<String>` in `BytecodeCompiler` distinguishes state machine names (→ `LoadState`) from struct variable names (→ `LoadGlobal/Local + FieldAccess`)

#### Display

Structs print as `User { name: alice, score: 10 }` — field names in BTreeMap alphabetical order, Text values without quotes.

#### Restrictions

- **No methods**: no `fn` attached to a struct; no `self` parameter
- **Struct type annotations require exact match**: `let u: User = User { ... }` is valid; `let u: User = Point { ... }` is a `TypeError`; annotation and literal struct name must agree
- **No nested struct literals**: `User { address: Address { city: "NY" } }` is unsupported
- **No generics**: `struct Box<T> { value: T }` is unsupported
- **No inheritance**: no extends/implements/traits
- **No nested field mutation**: `u.a.b = v` is unsupported
- **No expression-target field mutation**: `arr[0].f = v`, `get_user().f = v` unsupported; only plain mutable variable targets

---

### 4.11c Struct Field Mutation (Milestone 15B)

Mutable struct variables support direct field assignment and compound field assignment.

#### Plain field assignment

```kimin
struct User { name: Text, score: Number }

let mut u = User { name: "alice", score: 10 }
u.score = 20
u.name = "bob"
```

Rules:
- Target must be a plain identifier (not an expression): `ident.field = expr`
- The variable must be declared `let mut`
- The variable must hold a struct value
- The field must exist on the struct
- The assigned value must be compatible with the field's declared type

#### Compound field assignment

```kimin
u.score += 5
u.score -= 2
u.score *= 3
u.score /= 2
u.name += " kim"
```

Rules:
- Same variable and field requirements as plain assignment
- Old field value is read, RHS evaluated, binary op applied
- Result must be compatible with the field's declared type
- Follows the same operator rules as regular compound assignment (`+=` on Text concatenates)

#### Runtime semantics

- Struct is cloned out of the environment, the named field is updated in the cloned BTreeMap, and the updated struct is written back via `assign_existing` — all other fields are preserved
- Works through the full env chain (closures can mutate outer mutable structs)

#### Type rules

| Construct | Rule |
|-----------|------|
| `s.f = v` | `s` must be mutable `Type::Struct("S")`; `f` must exist; `v` must be compatible with field type |
| `s.f += v` | Same requirements; `check_binary(field_type, op, rhs_type)` must succeed; result assignable back to field type |

#### Bytecode

- `Instruction::SetField { name, field }` — compiler emits RHS value; VM pops value, loads struct variable by name, updates field, assigns back; disassembled as `SET_FIELD name.field`
- `Instruction::FieldCompoundAssign { name, field, op }` — compiler emits RHS; VM pops rhs, loads struct, reads old field value, applies `apply_compound_op(op, old, rhs)`, updates field, assigns back; disassembled as `FIELD_COMPOUND_ASSIGN name.field op=`

---

### 4.12A Conversion Builtins (Milestone 17A)

Three builtins convert between Kimin value types. All are intercepted before normal call dispatch; no `CALL` instruction is emitted.

| Builtin | Input | Output | Runtime behavior |
|---------|-------|--------|-----------------|
| `to_string(value)` | Any type | `Text` | Formats value using Kimin display rules |
| `to_number(text)` | `Text` | `Number` | Parses as f64; RuntimeError if not a valid number |
| `to_bool(text)` | `Text` | `Bool` | Accepts `"true"` or `"false"` exactly; RuntimeError otherwise |

Bytecode instructions: `TO_STRING`, `TO_NUMBER`, `TO_BOOL`.

---

### 4.12B Numeric Utility Builtins (Milestones 17B, 18A–18C)

All numeric builtins accept exactly one `Number` argument (except `pow` and `min`/`max` which take two). Unit types (`meters`, `seconds`, etc.) are distinct from `Number` and are rejected with `TypeError`.

#### Basic math

| Builtin | Return | Runtime behavior |
|---------|--------|-----------------|
| `abs(n)` | `Number` | Absolute value |
| `floor(n)` | `Number` | Round toward −∞ |
| `ceil(n)` | `Number` | Round toward +∞ |
| `round(n)` | `Number` | Round half away from zero |
| `min(a, b)` | `Number` | Minimum of two Numbers |
| `max(a, b)` | `Number` | Maximum of two Numbers |
| `sqrt(n)` | `Number` | Square root; RuntimeError if `n < 0` |
| `pow(base, exp)` | `Number` | `base^exp`; RuntimeError if result non-finite |

Bytecode: `ABS`, `FLOOR`, `CEIL`, `ROUND`, `MIN`, `MAX`, `SQRT`, `POW`.

#### Logarithm and exponential

| Builtin | Return | Domain | Runtime behavior |
|---------|--------|--------|-----------------|
| `ln(n)` | `Number` | `n > 0` | Natural logarithm; RuntimeError if `n ≤ 0` |
| `log2(n)` | `Number` | `n > 0` | Base-2 logarithm; RuntimeError if `n ≤ 0` |
| `log10(n)` | `Number` | `n > 0` | Base-10 logarithm; RuntimeError if `n ≤ 0` |
| `exp(n)` | `Number` | all finite | e^n; RuntimeError if result is non-finite (e.g. `exp(1000)`) |

Bytecode: `LN`, `LOG2`, `LOG10`, `EXP`.

#### Trigonometry (Milestone 18D)

All three trig builtins take a `Number` in **radians** and return `Number`. No degree mode. No inverse trig yet. No unit-aware angle overloads.

| Builtin | Return | Runtime behavior |
|---------|--------|-----------------|
| `sin(n)` | `Number` | Sine; uses `f64::sin`; RuntimeError if result non-finite |
| `cos(n)` | `Number` | Cosine; uses `f64::cos`; RuntimeError if result non-finite |
| `tan(n)` | `Number` | Tangent; uses `f64::tan`; RuntimeError if result non-finite |

Bytecode: `SIN`, `COS`, `TAN`.

**Note:** `tan` near asymptotes (e.g. `tan(π/2)`) produces a very large finite value in IEEE 754 double precision — the exact value of π/2 cannot be represented in `f64`, so the result is always finite. No special-casing is applied.

**Static type rules:** All trig builtins require `Type::Number` arg; return `Type::Number`. Unit types and all other types are `TypeError`.

**Examples:**
```kimin
print(sin(0))                              // 0
print(cos(0))                              // 1
print(tan(0))                              // 0
print(round(sin(1.5707963267948966)))      // 1   (≈ sin(π/2))
print(round(cos(3.141592653589793)))       // -1  (≈ cos(π))
print(round(tan(0.7853981633974483)))      // 1   (≈ tan(π/4))
```

---

### 4.12C Math Constants (Milestones 18E, 19A, 19B)

Four read-only builtin constants of type `Number`:

| Constant | Value | Source |
|----------|-------|--------|
| `PI` | 3.141592653589793 | `std::f64::consts::PI` |
| `E` | 2.718281828459045 | `std::f64::consts::E` |
| `TAU` | 6.283185307179586 | `std::f64::consts::TAU` (= 2π) |
| `PHI` | 1.618033988749895 | (1 + √5) / 2 (golden ratio) |

**Static type rules:**
- All four always have type `Type::Number`.
- Usable in any expression position.
- Assignment, compound assignment, shadowing (`let PHI`), function/method param, for-each var, and indexed for-each var are all `TypeError`.
- Calling `PHI()` is `TypeError: 'PHI' is a builtin constant, not a function`.
- PHI golden ratio property: `PHI² ≈ PHI + 1` → `round(PHI * PHI) = 3`, `round(PHI + 1) = 3`.

**Runtime rules:**
- Interpreter: intercepted before env lookup; each returns its `Value::Number` directly.
- `TAU = 2 * PI` — full-rotation trig: `sin(TAU) ≈ 0`, `cos(TAU) ≈ 1`.
- `PHI = (1.0 + 5.0_f64.sqrt()) / 2.0`
- No user-defined `const` declarations.

**Bytecode:** `PI`, `E_CONST`, `TAU`, `PHI` — each pushes its constant value, no env lookup.

**Bytecode:**
- `Instruction::Pi` → push `3.141592653589793`
- `Instruction::EConst` → push `2.718281828459045`
- Disassembler: `PI`, `E_CONST`

**Examples:**
```kimin
print(PI)                              // 3.141592653589793
print(E)                               // 2.718281828459045
print(round(sin(PI / 2)))             // 1
print(round(cos(PI)))                 // -1
print(round(ln(E)))                   // 1
```

---

### 4.12D Inverse Trigonometric Builtins (Milestone 18F)

| Builtin | Args | Return | Domain | Runtime behavior |
|---------|------|--------|--------|-----------------|
| `asin(n)` | `Number` | `Number` | [-1, 1] | `n.asin()` in radians; RuntimeError if `n < -1` or `n > 1` |
| `acos(n)` | `Number` | `Number` | [-1, 1] | `n.acos()` in radians; RuntimeError if `n < -1` or `n > 1` |
| `atan(n)` | `Number` | `Number` | any finite | `n.atan()` in radians |
| `atan2(y, x)` | `Number, Number` | `Number` | both finite | `y.atan2(x)` in radians; argument order is `(y, x)`; `atan2(0,0) = 0` (Rust f64) |

**Static type rules:**
- `asin`/`acos`/`atan`: exactly 1 `Number` arg; returns `Number`
- `atan2`: exactly 2 `Number` args; returns `Number`
- All four reject non-Number args (unit types, Bool, Text, etc.) with `TypeError`

**Argument order for atan2:**
- Source: `atan2(y, x)` — y first, x second
- Compiler emits y first, then x; VM pops x from stack first, then y
- Consistent with Rust's `f64::atan2` and standard math convention

**Bytecode:** `ASIN`, `ACOS`, `ATAN`, `ATAN2`

**Examples:**
```kimin
print(round(asin(1)))          // 2   (PI/2 ≈ 1.57 → rounds to 2)
print(round(acos(-1)))         // 3   (PI ≈ 3.14 → rounds to 3)
print(round(atan(1)))          // 1   (PI/4 ≈ 0.79 → rounds to 1)
print(round(atan2(1, 0)))      // 2   (PI/2 → rounds to 2)
print(round(atan2(0, -1)))     // 3   (PI → rounds to 3)
```

---

### 4.12E Euclidean Magnitude Builtin (Milestone 18G)

| Builtin | Args | Return | Runtime behavior |
|---------|------|--------|-----------------|
| `hypot(a, b)` | `Number, Number` | `Number` | √(a²+b²) via `f64::hypot`; RuntimeError for non-finite input/result |

**Static type rules:** 2 Number args → Number; unit types and all other types are `TypeError`.

**Compiler:** emit `a` then `b`; `Instruction::Hypot`. Stack: `[..., a, b]`; VM pops b first then a; calls `a.hypot(b)`.

**Symmetry:** `hypot(a, b) == hypot(b, a)`. Accepts negative inputs. `hypot(0, 0) = 0`.

**Bytecode:** `HYPOT`

**Examples:**
```kimin
print(hypot(3, 4))     // 5
print(hypot(5, 12))    // 13
print(hypot(-3, 4))    // 5
print(hypot(0, 0))     // 0
```

---

### 4.12F Clamp Builtin (Milestone 18H)

| Builtin | Args | Return | Runtime behavior |
|---------|------|--------|-----------------|
| `clamp(n, lo, hi)` | `Number, Number, Number` | `Number` | if n < lo → lo; if n > hi → hi; else → n. RuntimeError if lo > hi. |

**Static type rules:** 3 Number args → Number; unit types and all other types are `TypeError`.

**Compiler:** emit `n`, then `lo`, then `hi`; `Instruction::Clamp`. Stack: `[..., n, lo, hi]`; VM pops `hi` first, then `lo`, then `n`.

**Inclusive bounds:** `clamp(0, 0, 10) = 0`, `clamp(10, 0, 10) = 10`.

**Invalid bounds:** `clamp(n, 10, 0)` → `RuntimeError: clamp lower bound cannot be greater than upper bound`.

**Bytecode:** `CLAMP`

**Examples:**
```kimin
print(clamp(5, 0, 10))     // 5
print(clamp(-2, 0, 10))    // 0
print(clamp(12, 0, 10))    // 10
print(clamp(5, 5, 5))      // 5
```

---

### 4.12 Mutable Variables and Assignment

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

### 4.12 Compound Assignment Operators

Compound assignment provides shorthand for read-modify-write on `let mut` variables:

```kimin
let mut x: Number = 10
x += 5    // x is now 15
x -= 3    // x is now 12
x *= 2    // x is now 24
x /= 4    // x is now 6
```

**Syntax:**

```
compound_assign_stmt = IDENT ("+=" | "-=" | "*=" | "/=") expr
```

**Static rules:**

- `x` must be declared with `let mut` — assigning to an immutable variable is a `TypeError`.
- `x` must not be a state-typed variable — use `transition` instead.
- The type of the expression `x op rhs` is computed as if `x` were the left operand and `rhs` were the right operand of the corresponding binary operator:
  - `x += rhs`: applies `+` rules — `meters += meters` is valid; `meters += Number` is a `TypeError`
  - `x -= rhs`: applies `-` rules — same unit required
  - `x *= rhs`: applies `*` rules — `meters *= Number` is valid (scalar scaling)
  - `x /= rhs`: applies `/` rules — `meters /= Number` is valid (scalar division)
- The result type must be compatible with `x`'s declared type (same as the assignment compatibility rule).

**Error examples:**
```kimin
let x = 10
x += 1   // TypeError: cannot assign to immutable variable 'x'

let mut d: meters = 0
d += 10  // TypeError: operator '+' expected same-unit + same-unit, got meters + Number

state Door { open closed transition open -> closed }
let mut door: Door = Door.open
door += 1  // TypeError: state variables must be changed with transition, not compound assignment
```

**Simulate interaction:**

Compound assignment works inside `simulate` bodies and updates the outer mutable variable:

```kimin
let mut position: meters = 0
let velocity: meters = 10
let duration: seconds = 3
let dt: seconds = 1

simulate duration step dt {
  position += velocity
  print(position)
}
// 10 / 20 / 30
```

**Bytecode lowering:**

The bytecode compiler desugars `x += rhs` into `LoadGlobal/Local x` + `compile(rhs)` + `Add` + `StoreGlobal/Local x`. No new VM instructions are needed.

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
stmt            = state_decl | transition_stmt | simulate_stmt | while_stmt | for_stmt | break_stmt | continue_stmt | fn_decl | return_stmt | let_stmt | assign_stmt
                | compound_assign_stmt | index_assign_stmt | index_compound_assign_stmt | print_stmt | if_stmt | block | expr_stmt
state_decl      = "state" IDENT "{" (variant_decl | inner_transition)* "}"
variant_decl    = IDENT
inner_transition = "transition" IDENT "->" IDENT
transition_stmt = "transition" IDENT "->" IDENT
simulate_stmt   = "simulate" expr "step" expr "{" stmt* "}"
while_stmt      = "while" expr "{" stmt* "}"
for_stmt        = for_range_stmt | for_each_indexed_stmt | for_each_stmt
for_range_stmt  = "for" IDENT "in" "range" "(" expr "," expr ")" "{" stmt* "}"
for_each_stmt   = "for" IDENT "in" expr "{" stmt* "}"
for_each_indexed_stmt = "for" IDENT "," IDENT "in" expr "{" stmt* "}"
break_stmt      = "break"
continue_stmt   = "continue"
fn_decl         = "fn" IDENT "(" params ")" ("->" type_ann)? fn_body
return_stmt     = "return" expr?
let_stmt        = "let" "mut"? IDENT (":" type_ann)? "=" expr
assign_stmt          = IDENT "=" expr    (only when IDENT followed by single "=", not "==")
compound_assign_stmt = IDENT ("+=" | "-=" | "*=" | "/=") expr
index_assign_stmt    = IDENT "[" expr "]" "=" expr
index_compound_assign_stmt = IDENT "[" expr "]" ("+=" | "-=" | "*=" | "/=") expr
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
call            = primary ("(" args ")" | "[" expr "]" | "[" expr ".." expr "]")*
primary         = NUMBER | STRING | "true" | "false"
                | "[" expr ("," expr)* ","? "]"   (array literal, ≥1 element)
                | IDENT "." IDENT                  (state variant expression)
                | IDENT                            (variable reference)
                | "(" expr ")"
args            = (expr ("," expr)*)?
```

---

## Implementation Note: Bytecode IR and VM (Milestones 8A–10D)

Language semantics are defined by the tree-walk interpreter (`kimin run`). The bytecode compiler (`kimin bytecode`) and VM (`kimin vm`) are a separate experimental execution path.

### What the bytecode VM executes (M8A–10D)

- All core expressions: literals, arithmetic, comparisons, string concatenation, unary operators
- Variable access and mutation (globals and block-scoped locals via env-chain)
- Control flow: `if`/`else`, blocks with lexical scope
- Named function declarations and calls (including recursion)
- **Closures and free-variable capture** (M8F): `Value::BytecodeFunction { name, env }` carries its definition-site environment; functions close over enclosing locals and parameters
- **Dynamic/computed calls** (M8G): `make_getter()()` and `make_adder(2)(3)` both work; callee expression evaluated before arguments; any function-valued expression can be called
- **Compound assignment** (M9A): `x += expr`, `x -= expr`, `x *= expr`, `x /= expr` — desugared to `Load/op/Store` sequence; no new instructions
- **While loops** (M9B): `while <Bool-expr> { ... }` — lowered to `JumpIfFalse`/`Jump`/`BeginScope`/`EndScope`; no new VM instructions
- **Break and continue** (M9C): both desugar to `EndScope × N + Jump`; jump targets patched by `LoopContext`; no new VM instructions
- **For/range loops** (M9D): `for i in range(start, end) { ... }` — outer `BeginScope` holds loop var + sentinel; condition, body, increment, `Jump`; `continue` jumps to increment; `break` jumps to `EndScope(outer)`; no new VM instructions
- **For-each loops** (M13A): `for item in array_expr { ... }` — outer `BeginScope` holds `__kimin_foreach_iter_N` (array snapshot) and `__kimin_foreach_idx_N` (index counter); condition via `LEN`+`LESS`; inner `BeginScope` defines loop var via `INDEX`+`DefineLocal`; `continue` jumps to index-increment; `break` jumps to `EndScope(outer)`; no new VM instructions
- **Arrays** (M9E): `[e1, e2, e3]` → `ARRAY count`; `arr[i]` → `INDEX`; `len(arr)` → `LEN`; all three are new VM instructions
- **Array mutation by index** (M10A): `arr[i] = value` compiles index first, then value, then emits `SET_INDEX name`; VM looks up the existing array binding, validates the index, replaces the element, and writes the updated array back through the env chain
- **Array index compound assignment** (M10B): `arr[i] += value` and friends compile index first, then rhs, then emit `INDEX_COMPOUND_ASSIGN name op`; VM evaluates the index once, reads the old element, applies the compound operator, and writes the updated array back through the env chain
- **`push`/`pop` builtins** (M10C): `push(arr, value)` compiles value arg then emits `ARRAY_PUSH name`; `pop(arr)` emits `ARRAY_POP name`; no `CALL` instruction emitted; VM grows/shrinks the array in-place via env chain
- **Array slices** (M10D): `arr[start..end]` compiles array, start, and end, then emits `SLICE`; VM validates bounds and pushes a new independent `Value::Array`
- **State machine declarations** (`state Name { ... }`) — registers name, variants, and allowed transitions in the VM state registry
- **State variant values** (`Door.closed`) — validated against the registry, pushed as `Value::StateValue { state_name, variant_name }`
- **Transition statements** (`transition door -> opening`) — validates the edge exists in the registry, updates the variable in-place via env-chain walk
- **Simulate blocks** — see below

### What remains as `UNSUPPORTED` in the VM

No major language features remain unsupported. The only known limitation is:
- **Rc reference cycles** from recursive closures (a function that captures itself) — memory leak at runtime; programs run-and-exit so no crash occurs.

### Closure and environment model (M8F)

The VM uses an `Env` chain (same type as the tree-walk interpreter). Every scope operation creates or removes child `EnvRef` nodes:

- `BeginScope` → `Env::new_child(current_env)` 
- `EndScope` → `current_env = parent_ref()`
- `LoadFunction name` → pushes `Value::BytecodeFunction { name, env: Rc::clone(&current_env) }` — captures the definition-site env
- `Call { arg_count }` → creates `Env::new_child(captured_env)` as the call frame (lexical scoping, not dynamic)
- `Simulate { body_idx }` → creates `Env::new_child(current_env)` per iteration

All variable loads (`LoadGlobal`, `LoadLocal`) walk the chain from `current_env`. All stores (`StoreGlobal`, `StoreLocal`) call `assign_existing` which walks the chain to find and update the binding. `DefineGlobal` always binds in the root global env; `DefineLocal` binds in `current_env`.

### Dynamic call model (M8G)

All function calls use stack-based dispatch. The compiler emits:

1. Callee expression — pushes a `Value::BytecodeFunction` onto the stack
2. Arguments left-to-right — each pushes a value
3. `CALL arg_count` — pops N args (restores original order), pops callee, invokes

This handles all callee shapes uniformly:
- Named call: `add(1, 2)` → `LOAD_GLOBAL add, CONSTANT 1, CONSTANT 2, CALL 2`
- Returned closure: `make_getter()()` → `LOAD_GLOBAL make_getter, CALL 0, CALL 0`
- Curried call: `make_adder(2)(3)` → `LOAD_GLOBAL make_adder, CONSTANT 2, CALL 1, CONSTANT 3, CALL 1`

Non-function callees produce `RuntimeError: attempted to call non-function value of type ...`. Wrong arity produces `RuntimeError: function '...' expects N argument(s), got M`.

### Simulate block VM execution (M8E + M8F)

`simulate duration step dt { body }` lowers to:

1. Compile `duration` expression inline.
2. Compile `step` expression inline.
3. Compile `body` statements into a `SimulateChunk` stored in `BytecodeProgram.simulate_bodies`.
4. Emit `Instruction::Simulate { body_idx }` referencing the chunk by index.

At runtime, the VM:
- Pops `step` then `duration` from the stack.
- Validates: `step > 0`, `duration >= 0`.
- Loops `floor(duration / step)` times:
  - Creates `Env::new_child(current_env)` — body can read/write block-local outer variables.
  - Defines `time = i * step` in the child env.
  - Executes the body chunk (with `is_fn` passed through for `return` propagation).
- Outer mutable variables and state machines persist across iterations via env-chain assignment.
- Body-local `let` bindings are fresh per iteration.

### Bytecode IR structures

```
BytecodeProgram {
  main: Chunk,                          // top-level instructions
  functions: Vec<FunctionChunk>,        // one per fn decl, in source order
  simulate_bodies: Vec<SimulateChunk>,  // one per simulate block, in source order
}

FunctionChunk {
  name: String,
  params: Vec<String>,
  arity: usize,
  chunk: Chunk,
}

SimulateChunk {
  name: String,   // e.g. "simulate#0"
  chunk: Chunk,
}
```

### Instructions

| Instruction | Meaning |
|-------------|---------|
| `LOAD_FUNCTION name` | Push `Value::BytecodeFunction { name, env: current_env }` — captures definition-site env |
| `CALL arg_count` | Pop N args and callee from stack; invoke callee(args); push return value |
| `DEFINE_STATE name variants=[...] transitions=[...]` | Register state machine metadata in VM registry; no stack effect |
| `LOAD_STATE state.variant` | Validate and push `Value::StateValue { state_name, variant_name }` |
| `TRANSITION var -> target` | Read var, validate transition edge, update var in-place via env-chain |
| `SIMULATE body_idx` | Pop step and duration; loop body_idx chunk floor(dur/step) times |
| `ARRAY count` | Pop `count` values (top = last element), reverse, push `Value::Array` |
| `INDEX` | Pop index and array; validate integer/bounds; push element |
| `SLICE` | Pop end, start, and array; validate integer/bounds; push independent array copy |
| `LEN` | Pop array; push `Value::Number(len)` |
| `SET_INDEX name` | Pop new_value and index; look up `name` in env chain; clone array; update element; assign back |
| `INDEX_COMPOUND_ASSIGN name op` | Pop rhs and index; read old element at index; apply op(old, rhs); clone array; update element; assign back |
| `ARRAY_PUSH name` | Pop new_value; look up `name` in env chain; clone array; append element; assign back; push Nil |
| `ARRAY_POP name` | Look up `name` in env chain; clone array; pop last element; assign back; push popped element |
