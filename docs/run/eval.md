# Run in Eval mode

Use `eval` command to evaluate code snippets from CLI:

```bash
$ cr eval 'echo |demo'
1
took 0.07ms: nil
```

```bash
$ cr eval 'echo "|spaced string demo"'
spaced string demo
took 0.074ms: nil
```

## Multi-line Code

You can run multiple expressions:

```bash
cr eval '
-> (range 10)
  map $ fn (x)
    * x x
'
# Output: calcit version: 0.5.25
# took 0.199ms: ([] 0 1 4 9 16 25 36 49 64 81)
```

## Working with Context Files

Eval can access definitions from a loaded program:

```bash
# Load from specific file and eval with its context
cr demos/compact.cirru eval 'range 3'
# Output: ([] 0 1 2)

# Use let bindings
cr demos/compact.cirru eval 'let ((x 1)) (+ x 2)'
# Output: 3
```

You can load external modules with repeatable `--dep` options:

```bash
cr demos/compact.cirru eval --dep ~/.config/calcit/modules/respo.calcit/ -- 'ns app.demo $ :require respo.util.detect :refer $ element?\n\nelement? nil'
```

If the first expression in a snippet is `ns`, its `:require` rules are merged into runtime `ns app.main`, so imported symbols can be used in the same snippet.

## Type Checking in Eval

Type annotations and static checks work in eval mode:

```bash
# Type mismatch will cause error
cr demos/compact.cirru eval 'let ((x 1)) (assert-type x :string) x'
# Error: Type mismatch...

# Correct type passes
cr demos/compact.cirru eval 'let ((x 1)) (assert-type x :number) x'
# Output: 1
```

## Common Patterns

### Quick Calculations

```bash
cr eval '+ 1 2 3 4'
# Output: 10

cr eval 'apply * $ range 1 6'
# Output: 120  ; factorial of 5
```

### Testing Expressions

```bash
cr eval '&list:nth ([] :a :b :c) 1'
# Output: :b

cr eval '&map:get ({} (:x 1) (:y 2)) :x'
# Output: 1
```

### Exploring Functions

```bash
# Check function signature
cr eval 'type-of range'
# Output: :fn

# Test with sample data
cr eval '-> (range 5) (map inc) (filter (fn (x) (> x 2)))'
# Output: ([] 3 4 5)
```

## Important Notes

### Syntax Considerations

- **No extra brackets**: Cirru syntax doesn't need outer parentheses at top level
  - ✅ `cr eval 'range 3'`
  - ❌ `cr eval '(range 3)'` (adds extra nesting)

- **Let bindings**: Use paired list format `((name value))`
  - ✅ `let ((x 1)) x`
  - ❌ `let (x 1) x` (triggers "expects pairs in list for let" error)

### Error Diagnostics

- Type warnings cause eval to fail (intentional safety feature)
- Check `.calcit-error.cirru` for complete stack traces
- Use `cr cirru parse-oneliner` to debug parse issues

### Query Examples

Use `cr query examples` to see usage examples:

```bash
cr demos/compact.cirru query examples calcit.core/let
cr demos/compact.cirru query examples calcit.core/defn
```

For markdown snippet validation (`docs check-md`), see [CLI Options](./cli-options.md#markdown-code-checking).
