---
title: "Cirru Syntax Essentials"
summary: "Cirru/Calcit 语法基础：AST 嵌套、$ 和 ,、字符串、集合构造、CLI quote 边界与高频语义陷阱"
scope: "core"
kind: "reference"
category: "syntax"
aliases:
  - "cirru syntax"
  - "cirru ast"
  - "dollar operator"
  - "comma operator"
  - "string literals"
  - "quote code data boundary"
  - "empty collection constructor"
entry_for:
  - "calcit cirru parse"
  - "calcit cirru format"
  - "calcit cirru show-guide"
---

## Cirru Syntax Essentials

### 1. Indentation and Parentheses Build the AST

Cirru uses **2-space indentation** to represent nested structures. Do not use tabs. A line is already an expression; parentheses are only for creating an inline child expression:

```cirru
defn add (a b)
  &+ a b
```

Equivalent JSON:

```json
["defn", "add", ["a", "b"], ["&+", "a", "b"]]
```

Keep these shapes in mind:

```text
a b c          => ["a", "b", "c"]
a (b c) d      => ["a", ["b", "c"], "d"]
range 3        => ["range", "3"]
(range 3)      => [["range", "3"]]  # extra call layer, usually wrong
```

Calcit evaluates a non-special list by treating its first child as the operator. Therefore, an extra outer pair of parentheses is not decorative: it can call the result of the inner expression.

### 2. The `$` Operator (Single-Child Expand)

`$` creates a **single nested expression** from everything to its right on the same line. Chained `$` associates from right to left:

```cirru
do
  ; Without $: explicit nesting
  let
      x 1
    str x
  ; Multiple $ chain right-to-left
  str $ &+ 1 2
  ; Equivalent to: (str (&+ 1 2))
```

**Rule**: `a $ b c` → `["a", ["b", "c"]]`

```text
a $ b $ c d    => ["a", ["b", ["c", "d"]]]
x (f a)        => ["x", ["f", "a"]]
x $ (f a)      => ["x", [["f", "a"]]]  # extra layer, usually wrong
```

Do not add `$` immediately before an already-parenthesized expression unless the extra list is intentional.

### 3. The `|` Prefix (String Literals)

`|` marks a **string literal**:

```cirru
println |hello
println |hello-world
println "|hello world with spaces"
```

- `|hello` → `"hello"` (string, not symbol)
- Without `|`: `hello` is a symbol/identifier
- For strings with spaces: `"|hello world"`
- Double quotes only keep a token together; they do not make it a Calcit string by themselves. `"hello"` is still the symbol `hello`.
- Keep multiline text in one token with escapes such as `"|line 1\nline 2"`; do not continue a string across source lines.

### 4. The `,` Operator (Splice Values into the Parent)

An indented line normally becomes a child list. A leading `,` splices the rest of that line into the parent instead. This is how a standalone value remains a leaf rather than becoming a zero-argument call:

```text
a
  b c
  , d

=> ["a", ["b", "c"], "d"]
```

Without the comma, the final line would produce `["d"]`. The same rule matters when a function or `let` body returns a local value:

```cirru.no-check
fn (x)
  , x
```

### 5. Collection Constructors Are Operators

`[]`, `{}`, and `#{}` are Calcit constructor symbols rather than delimiters around source text. As a same-line argument, bare `[]` is the constructor function, not an empty list value:

```text
type-of []       => :fn
type-of $ []     => :list
```

For an empty collection argument, call the constructor explicitly with `([])`, or bind it first:

```cirru.no-check
let
    init $ []
  foldl xs init $ fn (acc x)
    , acc
```

Calcit collection functions use collection-first order: `map xs f`, `filter xs pred`, and `foldl xs init f`.

### 6. Quasiquote, Unquote, Unquote-Splicing

For macros:

- `quasiquote` or backtick: template
- `~` (unquote): insert evaluated value
- `~@` (unquote-splicing): splice list contents

```cirru.no-check
defmacro when-not (cond & body)
  quasiquote $ if (not ~cond)
    do ~@body
```

JSON equivalent:

```json
["defmacro", "when-not", ["cond", "&", "body"], ["quasiquote", ["if", ["not", "~cond"], ["do", "~@body"]]]]
```

## LLM Guidance & Optimization

To ensure high-quality code generation for Calcit, follow these rules:

### 1. Mandatory `|` Prefix for Strings

LLMs often forget the `|` prefix. **Always** use `|` for string literals, even short ones.

- ❌ `println "hello"`
- ✅ `println |hello`
- ✅ `println "|hello with spaces"`

### 2. Functional `let` Binding

`let` bindings must be a list of pairs `((name value))`. Single brackets `(name value)` are invalid.

- ❌ `let (x 1) x`
- ✅ `let ((x 1)) x`
- ✅ **Preferred**: Use multi-line for clarity:
  ```cirru.no-run
  let
      x 1
      y 2
    + x y
  ```

### 3. Parse Success Is Not Semantic Success

`calcit cirru parse -e --validate '<expr>'` verifies Cirru tokens and shows the AST shape. It does not prove that Calcit will accept the resulting call structure. Forms such as `(range 3)`, `x $ (f a)`, `let (x 1) ...`, or a bare indented return value can parse successfully and still be wrong.

After inspecting the JSON shape, use `calcit eval`, `calcit --check-only`, or the project's tests to verify semantics.

### 4. Arity Awareness

Calcit uses strict arity checking. Many core functions like `+`, `-`, `*`, `/` have native counterparts `&+`, `&-`, `&*`, `&/` which are binaries (2 arguments). The standard versions are often variadic macros.

- Use `&+`, `&-`, etc. in tight loops or when 2 args are guaranteed.

### 5. No Inline Types in Parameters

Calcit keeps parameter lists structural and declares types in function schemas rather than inline parameter metadata.

- ❌ `defn add (a :number) ...`
- ✅ Use function schema for parameter types (`:schema` on top-level defs, `hint-fn` for local `fn`).
- ✅ Return types can be specified with `hint-fn` or a **trailing label** after parameters:

```cirru
let
    ; hint-fn only: declare arg and return types
    square $ fn (n)
      hint-fn $ {} (:args ([] :number)) (:return :number)
      &* n n
    ; trailing return type label
    get-pi $ fn () :number
      , 3.14159
    ; mixed: hint-fn for args, trailing label for return
    add $ fn (a b) :number
      hint-fn $ {} (:args ([] :number :number))
      + a b
  do
    assert= 25 $ square 5
    assert= 3.14159 $ get-pi
    assert= 7 $ add 3 4
```

For namespace-level definitions, attach schema separately, for example:

```cirru
defn square (n)
  &* n n

:: :fn $ {} (:args $ [] :number) (:return :number)
```

### 6. `$` and `,` Usage

- Use `$` to avoid parentheses on the same line.
- Use `, value` when an indented value must be spliced into its parent instead of becoming a one-child call list. Do not use it to delimit `cond` or `case` pairs; those branches must remain child lists.

### 7. Common Patterns

#### Function Definition

```cirru.no-check
defn function-name (arg1 arg2)
  body-expression
```

#### Let Binding

```cirru
let
    x 1
    y $ &+ x 2
  &* x y
```

#### Conditional

```cirru.no-check
if condition
  then-branch
  else-branch
```

#### Multi-branch Cond

```cirru.no-check
cond
  (test1) result1
  (test2) result2
  true default-result
```

## JSON Input Format

Input is automatically detected as JSON when it starts with `[` (Cirru JSON is always arrays, never objects). No flags needed.

For `calcit edit`, `calcit tree`, and cursor mutation commands, the primary Cirru EDN form uses `quote` as a code/data boundary. It must wrap exactly one AST node and is removed before writing source:

```text
symbol leaf:    quote new-name
string leaf:    quote |hello
spaced string:  quote "|hello world"
expression:     quote $ println |hello
empty list/map: quote $ []    /    quote $ {}
```

`quote println |hello` is invalid because it supplies two payloads. Use `quote $ println |hello` or `quote (println |hello)`.

When providing JSON:

1. **Everything is arrays or strings**: `["defn", "name", ["args"], ["body"]]`
2. **Numbers as strings**: `["&+", "1", "2"]` not `["&+", 1, 2]`
3. **Preserve prefixes**: `"|string"`, `"~var"`, `"~@list"`
4. **No objects**: JSON `{}` cannot be converted to Cirru

## Common Mistakes

- Wrong `println hello` or `println "hello"`; correct `println |hello`. Without the `|`, the argument is a symbol.
- Wrong top-level `(range 3)`; correct `range 3`. The former has an extra call layer.
- Wrong `x $ (f a)`; correct `x $ f a` or `x (f a)`. Do not combine `$` with redundant parentheses.
- Wrong bare indented return `x`; correct `, x`. The bare line is a one-child call list.
- Wrong `let (x 1) ...`; correct `let ((x 1)) ...`, or use the documented multiline binding indentation.
- Wrong empty-list argument `foldl xs [] f`; correct `foldl xs ([]) f`, or bind `init $ []` first.
- Wrong collection order `map f xs`; correct Calcit order `map xs f`.
- Wrong `$ a b c` at the beginning of a line; correct `a b c`. A line is already an expression.
- Wrong `a$b`; correct `a $ b`. Operators require token-separating spaces.
- Wrong JSON AST `["&+", 1, 2]`; correct `["&+", "1", "2"]`. AST leaves are strings.
- Wrong tabs for indentation; use 2 spaces.
