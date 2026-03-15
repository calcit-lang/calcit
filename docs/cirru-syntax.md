## Cirru Syntax Essentials

### 1. Indentation = Nesting

Cirru uses **2-space indentation** to represent nested structures:

```cirru
defn add (a b)
  &+ a b
```

Equivalent JSON:

```json
["defn", "add", ["a", "b"], ["&+", "a", "b"]]
```

### 2. The `$` Operator (Single-Child Expand)

`$` creates a **single nested expression** on the same line:

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

### 4. The `,` Operator (Expression Terminator)

`,` forces the **end of current expression**, starting a new sibling:

```cirru
; Without comma - ambiguous
if true 1 2

; With comma - clear structure
if true
  , 1
  , 2
```

Useful in `cond`, `case`, `let` bindings:

```cirru
let
    x (- 0 3)
  ; cond tests conditions in sequence, returning first matching result
  cond
    (&< x 0) |negative
    (&= x 0) |zero
    true |positive
```

### 5. Quasiquote, Unquote, Unquote-Splicing

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
[
  "defmacro",
  "when-not",
  ["cond", "&", "body"],
  ["quasiquote", ["if", ["not", "~cond"], ["do", "~@body"]]]
]
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

### 3. Arity Awareness

Calcit uses strict arity checking. Many core functions like `+`, `-`, `*`, `/` have native counterparts `&+`, `&-`, `&*`, `&/` which are binaries (2 arguments). The standard versions are often variadic macros.

- Use `&+`, `&-`, etc. in tight loops or when 2 args are guaranteed.

### 4. No Inline Types in Parameters

Calcit **does not** support Clojure-style `(defn name [^Type arg] ...)`.

- ❌ `defn add (a :number) ...`
- ✅ Use function schema for parameter types (`:schema` on top-level defs, `hint-fn` for local `fn`).
- ✅ Return types can be specified with `hint-fn` or a **trailing label** after parameters:

```cirru
let
  ; Local helper function
    square $ defn square (n)
      hint-fn $ {} (:args ([] :number)) (:return :number)
      &* n n
    ; Return type as trailing label
    get-pi $ defn get-pi () :number
      , 3.14159
    ; Mixed style
    add $ defn add (a b) :number
      hint-fn $ {} (:args ([] :number :number))
      + a b
  [] (square 5) (get-pi) (add 3 4)
```

For namespace-level definitions, attach schema separately, for example:

```cirru
defn square (n)
  &* n n

:: :fn $ {} (:args $ [] :number) (:return :number)
```

### 5. `$` and `,` Usage

- Use `$` to avoid parentheses on the same line.
- Use `,` to separate multiline pairs in `cond` or `case` if indentation alone feels ambiguous.

### 6. Common Patterns

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

## JSON Format Rules

When using `-j` or `--json-input`:

1. **Everything is arrays or strings**: `["defn", "name", ["args"], ["body"]]`
2. **Numbers as strings**: `["&+", "1", "2"]` not `["&+", 1, 2]`
3. **Preserve prefixes**: `"|string"`, `"~var"`, `"~@list"`
4. **No objects**: JSON `{}` cannot be converted to Cirru

## Common Mistakes

| ❌ Wrong                | ✅ Correct         | Reason                                                    |
| ----------------------- | ------------------ | --------------------------------------------------------- |
| `println hello`         | `println \|hello`  | Missing `\|` for string                                   |
| `$ a b c` at line start | `a b c`            | A line is an expression, no need of `$` for extra nesting |
| `a$b`                   | `a $ b`            | Missing space around `$`                                  |
| `["&+", 1, 2]`          | `["&+", "1", "2"]` | Numbers in syntax tree must be strings in JSON            |
| Tabs for indent         | 2 spaces           | Cirru requires spaces                                     |
