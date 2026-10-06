---
title: "Macros"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "defmacro"
  - "macro expansion"
  - "quote"
---
# Macros

Calcit macros extend the language by transforming syntax trees during preprocessing. A `defmacro` block returns lists, symbols, and literals:

## 条件与短路求值

`if` 只把 `nil`、`false` 和 `&unit` 视为假值；`0`、负零、空字符串、空集合和 `Option :none` 都是真值。
条件只求值一次，只执行选中的分支；省略 else 时，假值分支返回 `nil`。
`or` 返回遇到的第一个真值，全部为假时返回最后一个值；`and` 遇到假值时返回 `false`，否则返回最后一个值。
这些是 Calcit 语义，不使用 JavaScript 的自动布尔转换。

```cirru
assert= |then $ if 0 |then |else
assert= |then $ if | |then |else
assert= |else $ if &unit |then |else
assert= 0 $ or 0 7
assert= | $ or | |fallback
assert= |done $ and 0 | |done
assert= false $ and nil |unselected
```

已证明为 `Bool` 的条件可直接生成布尔分支；其他条件仍遵守以上真假值规则。
JS 的 `null`/`undefined` 分别承载 `nil`/`Unit`；真假值判断不是宿主对象的受检解码，不能替代 FFI 类型边界。

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
defmacro twice (item)
  &let
    v (gensym |v)
    quasiquote
      &let (~v ~item)
        + ~v ~v
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

Macros operate on Calcit syntax and values; use Calcit examples and macro-expansion tools rather than assuming forms from another language.

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
$ calcit eval 'println $ format-to-cirru $ macroexpand-all $ quote $ let ((a 1) (b 2)) (+ a b)'

&let (a 1)
  &let (b 2)
    + a b

```

`format-to-cirru` and `format-to-lisp` are 2 custom code formatters:

```
$ calcit eval 'println $ format-to-lisp $ macroexpand-all $ quote $ let ((a 1) (b 2)) (+ a b)'

(&let (a 1) (&let (b 2) (+ a b)))
```

`macroexpand`, `macroexpand-1`, and `macroexpand-all` also print the expansion chain on stderr when nested macros are involved (for example `m1 -> m2 -> m3`). This is useful when a call site expands through helper macros before reaching final syntax.

## 限制

- 当前 WASM 标量条件仍按非零值判断；上述非 Bool 条件的完整真假值契约尚未对齐，跨目标代码暂优先使用明确的 Bool 条件。

The syntax `macroexpand` only expand syntax tree once:

```
$ calcit eval 'println $ format-to-cirru $ macroexpand $ quote $ let ((a 1) (b 2)) (+ a b)'

&let (a 1)
  let
      b 2
    + a b
```
