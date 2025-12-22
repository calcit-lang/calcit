//! Cirru subcommand handlers
//!
//! Handles: cr cirru parse, format, parse-edn, show-guide

use calcit::cli_args::{CirruCommand, CirruSubcommand};

pub fn handle_cirru_command(cmd: &CirruCommand) -> Result<(), String> {
  match &cmd.subcommand {
    CirruSubcommand::Parse(opts) => handle_parse(&opts.code),
    CirruSubcommand::Format(opts) => handle_format(&opts.json),
    CirruSubcommand::ParseEdn(opts) => handle_parse_edn(&opts.edn),
    CirruSubcommand::ShowGuide(_) => handle_show_guide(),
  }
}

fn cirru_to_json(cirru: &cirru_parser::Cirru) -> serde_json::Value {
  match cirru {
    cirru_parser::Cirru::Leaf(s) => serde_json::Value::String(s.to_string()),
    cirru_parser::Cirru::List(items) => serde_json::Value::Array(items.iter().map(cirru_to_json).collect()),
  }
}

fn json_to_cirru(json: &serde_json::Value) -> Result<cirru_parser::Cirru, String> {
  match json {
    serde_json::Value::String(s) => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from(s.as_str()))),
    serde_json::Value::Array(arr) => {
      let items: Result<Vec<cirru_parser::Cirru>, String> = arr.iter().map(json_to_cirru).collect();
      Ok(cirru_parser::Cirru::List(items?))
    }
    serde_json::Value::Number(n) => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from(n.to_string()))),
    serde_json::Value::Bool(b) => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from(b.to_string()))),
    serde_json::Value::Null => Ok(cirru_parser::Cirru::Leaf(std::sync::Arc::from("nil"))),
    serde_json::Value::Object(_) => Err("JSON objects cannot be converted to Cirru".to_string()),
  }
}

fn handle_parse(code: &str) -> Result<(), String> {
  // Check if input looks like JSON
  let trimmed = code.trim_start();
  if trimmed.starts_with('[') {
    return Err("Input appears to be JSON format, not Cirru code. This tool is for parsing Cirru syntax only.".to_string());
  }

  let cirru_data = cirru_parser::parse(code).map_err(|e| format!("Failed to parse Cirru code: {e}"))?;

  let json_result = if cirru_data.len() == 1 {
    cirru_to_json(&cirru_data[0])
  } else {
    serde_json::Value::Array(cirru_data.iter().map(cirru_to_json).collect())
  };

  let json_str = serde_json::to_string_pretty(&json_result).map_err(|e| format!("Failed to serialize JSON: {e}"))?;

  println!("{json_str}");

  Ok(())
}

fn handle_format(json_str: &str) -> Result<(), String> {
  let json_data: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;

  let cirru_data = json_to_cirru(&json_data)?;

  let cirru_code = cirru_parser::format(&[cirru_data], true.into()).map_err(|e| format!("Failed to format Cirru: {e}"))?;

  println!("{cirru_code}");

  Ok(())
}

fn handle_parse_edn(edn_str: &str) -> Result<(), String> {
  let edn = cirru_edn::parse(edn_str).map_err(|e| format!("Failed to parse Cirru EDN: {e}"))?;

  let json_value = serde_json::to_value(&edn).map_err(|e| format!("Failed to convert EDN to JSON: {e}"))?;

  let json_str = serde_json::to_string_pretty(&json_value).map_err(|e| format!("Failed to serialize JSON: {e}"))?;

  println!("{json_str}");

  Ok(())
}

fn handle_show_guide() -> Result<(), String> {
  println!("{CIRRU_SYNTAX_GUIDE}");
  Ok(())
}

const CIRRU_SYNTAX_GUIDE: &str = r#"# Cirru Syntax Guide for LLM Code Generation

This guide helps LLMs generate correct Cirru syntax for use with `cr edit` commands.

## Input Formats

`cr edit` commands accept two input formats:
- **Cirru text** (default): Human-readable indented syntax
- **JSON**: Nested arrays representing the syntax tree

### Quick Examples

```bash
# Cirru input (default)
cr edit def my.ns/my-fn -f code.cirru

# JSON input (explicit)
cr edit def my.ns/my-fn -j '["defn","my-fn",["x"],["&+","x","1"]]'
cr edit def my.ns/my-fn -f code.json --json-input
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
["defmacro", "when-not", ["cond", "&", "body"],
  ["quasiquote", ["if", ["not", "~cond"], ["do", "~@body"]]]]
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

| Wrong | Correct | Reason |
|-------|---------|--------|
| `println hello` | `println \|hello` | Missing `\|` for string |
| `$ a b c` at line start | `a $ b c` | `$` needs left operand |
| `["&+", 1, 2]` | `["&+", "1", "2"]` | Numbers must be strings in JSON |
| Tabs for indent | 2 spaces | Cirru requires spaces |

## Testing Your Code

Always verify with `query def`:

```bash
# Create definition
cr edit def my.ns/my-fn -f code.cirru

# Verify structure
cr query def my.ns/my-fn

# Explore specific path
cr query at my.ns/my-fn -p "1,0" -d 3
```

## Shell Escaping Tips

When passing Cirru/JSON inline:
- Use **single quotes** to preserve `|`, `$`, `~`
- Or use **file input** (`-f`) to avoid shell interpretation
- Example: `-j '["println", "|hello"]'`
"#;
