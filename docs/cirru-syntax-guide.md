# Cirru Syntax Guide for LLM Code Generation

This guide helps LLMs generate correct Cirru syntax for use with `cr edit` commands.

## Input Formats

`cr edit` commands accept two input formats:

- **Cirru text** (default): Human-readable indented syntax
- **JSON**: Nested arrays representing the syntax tree

### Quick Examples

```bash
# Cirru input (default)
cr edit upsert-def my.ns my-fn -f code.cirru

# JSON input (explicit)
cr edit upsert-def my.ns my-fn -j '["defn","my-fn",["x"],["&+","x","1"]]'
cr edit upsert-def my.ns my-fn -f code.json --json-input
```

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
; Without $: explicit nesting
let
    x 1
  println x

; With $: inline nesting
let (x 1)
  println x

; Multiple $ chain right-to-left
println $ str $ &+ 1 2
; Equivalent to: (println (str (&+ 1 2)))
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
cond
    &< x 0
    , |negative      ; comma separates condition from result
  (&= x 0) |zero
  true |positive
```

### 5. Quasiquote, Unquote, Unquote-Splicing

For macros:

- `quasiquote` or backtick: template
- `~` (unquote): insert evaluated value
- `~@` (unquote-splicing): splice list contents

```cirru
defmacro when-not (cond & body)
  quasiquote $ if (not ~cond)
    do ~@body
```

JSON equivalent:

```json
["defmacro", "when-not", ["cond", "&", "body"], ["quasiquote", ["if", ["not", "~cond"], ["do", "~@body"]]]]
```

### 6. Common Patterns

#### Function Definition

```cirru
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

```cirru
if condition
  then-branch
  else-branch
```

#### Multi-branch Cond

```cirru
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

| ❌ Wrong                | ✅ Correct         | Reason                          |
| ----------------------- | ------------------ | ------------------------------- | --------- | ------------ |
| `println hello`         | `println           | hello`                          | Missing ` | ` for string |
| `$ a b c` at line start | `a $ b c`          | `$` needs left operand          |
| `["&+", 1, 2]`          | `["&+", "1", "2"]` | Numbers must be strings in JSON |
| Tabs for indent         | 2 spaces           | Cirru requires spaces           |

## Testing Your Code

Always verify with `query read-def`:

```bash
# Create definition
cr edit upsert-def my.ns my-fn -f code.cirru

# Verify structure
cr query read-def my.ns my-fn

# Explore specific path
cr query read-at my.ns my-fn -p "1,0" -d 3
```

## Shell Escaping Tips

When passing Cirru/JSON inline:

- Use **single quotes** to preserve `|`, `$`, `~`
- Or use **file input** (`-f`) to avoid shell interpretation
- Example: `-j '["println", "|hello"]'`
