# Macros

Like Clojure, Calcit uses macros to support new syntax. And macros ared evaluated during building to expand syntax tree. A `defmacro` block returns list and symbols, as well as literals:

## Quick Recipes

- **Define**: `defmacro my-macro (x) ...`
- **Template**: `quasiquote $ if ~x ~y ~z`
- **Splice**: `~@xs` to unpack a list into the template
- **Fresh Symbols**: `gensym |name` or `with-gensyms (a b) ...`
- **Local Bindings**: `&let (v ~item) ...`

```cirru
defmacro noted (x0 & xs)
  if (empty? xs) x0
    last xs
```

A normal way to use macro is to use `quasiquote` paired with `~x` and `~@xs` to insert one or a span of items. Also notice that `~x` is internally expanded to `(~ x)`, so you can also use `(~ x)` and `(~@ xs)` as well:

```cirru
defmacro if-not (condition true-branch ? false-branch)
  quasiquote $ if ~condition ~false-branch ~true-branch
```

To create new variables inside macro definitions, use `(gensym)` or `(gensym |name)`:

```cirru
defmacro case (item default & patterns)
  &let
    v (gensym |v)
    quasiquote
      &let (~v ~item)
        &case ~v ~default ~@patterns
```

For macros that need multiple fresh symbols, use `with-gensyms` from `calcit.core`:

```cirru
defmacro swap! (a b)
  with-gensyms (tmp)
    quasiquote
      let ((~tmp ~a))
        reset! ~a ~b
        reset! ~b ~tmp
```

Calcit was not designed to be identical to Clojure, so there are many details here and there.

### Macros and Static Analysis

Macros expand before type checking, so generated code is validated:

```cirru.no-check
defmacro assert-positive (x)
  quasiquote
    if (< ~x 0)
      raise "|Value must be positive"
      ~x

; After expansion, type checking applies to generated code
defn process (n)
  hint-fn $ {} (:args ([] :number))
  assert-positive n  ; Macro expands, then type-checked
```

**Important**: Macro-generated functions (like loop's `f%`) are automatically excluded from certain static checks (e.g., recur arity) to avoid false positives. Functions with `%`, `$`, or `__` prefix are treated as compiler-generated.

### Best Practices

- **Use gensym for local variables**: Prevents name collision
- **Keep macros simple**: Complex logic belongs in functions
- **Document macro behavior**: Include usage examples
- **Test macro expansion**: Use `macroexpand-all` to verify output
- **Avoid side effects**: Macros should only transform syntax

### Debug Macros

Use `macroexpand-all` for debugging:

```
$ cr eval 'println $ format-to-cirru $ macroexpand-all $ quote $ let ((a 1) (b 2)) (+ a b)'

&let (a 1)
  &let (b 2)
    + a b

```

`format-to-cirru` and `format-to-lisp` are 2 custom code formatters:

```
$ cr eval 'println $ format-to-lisp $ macroexpand-all $ quote $ let ((a 1) (b 2)) (+ a b)'

(&let (a 1) (&let (b 2) (+ a b)))
```

`macroexpand`, `macroexpand-1`, and `macroexpand-all` also print the expansion chain on stderr when nested macros are involved (for example `m1 -> m2 -> m3`). This is useful when a call site expands through helper macros before reaching final syntax.

The syntax `macroexpand` only expand syntax tree once:

```
$ cr eval 'println $ format-to-cirru $ macroexpand $ quote $ let ((a 1) (b 2)) (+ a b)'

&let (a 1)
  let
      b 2
    + a b
```
