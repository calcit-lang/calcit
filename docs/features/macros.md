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

`if`、`when`、`when-not`、`if-not`、`and`、`or` 的条件在静态检查与运行时都严格要求为 `Bool`；`nil`、`&unit`、`0`、字符串、集合、`Option :none` 与未证明的 `Dynamic` 都会被拒绝（静态报 `E_NON_BOOL_CONDITION`）。
条件只求值一次，只执行选中的分支：

- `if condition then-expr else-expr`：省略 `else-expr` 时假分支返回 `&unit`（`Unit`，仅允许用于无需值的语句位置；需要值请显式返回 `Option :some` / `Option :none`）。
- `when`、`when-not` 与两参数 `if-not` 始终返回 `&unit`（`Unit`）；三参数 `if-not` 在条件为 `false` 时求值第二参数、为 `true` 时求值第三参数。
- `and` 与 `or` 按从左到右短路求值并仅接受和返回 `Bool`。若需做缺省值回退，请使用 `Option .unwrap-or`、`if-let` 或 `match`（旧式 `either` 已标记 `:deprecated`）。
- 这些规则在 native、JS 与 WASM/WASI 之间一致；JavaScript 生成代码在必要时调用 `assert_bool` 守卫，不使用 JS 自动真假转换。

条件的静态类型已证明为 Number、String、List、Map、Set 等非布尔类型时，检查会报告 `E_CONSTANT_TRUTHY_CONDITION` 并提示改写为 `>`、`=`、`empty?` 或 `blank?`。可选值优先使用 `Option<T>`（`.some?` / `.none?` / `match`），仅遗留 `nil` 边界再用 `non-nil?` / `nil?` 转为 `Bool`，`Dynamic` 请先用 `(bool? x)` 或 `(= x true)` 收窄。

```cirru
assert= |then $ if true |then |else
assert= |else $ if false |then |else
assert= false $ if (&= 0 1) true false
assert= false $ and (&> 2 3) true
assert= true $ or false (&= 1 1)
assert= 7 $ .unwrap-or (assert-type (Option :none) (Option Number)) 7
assert= |fallback $ .unwrap-or (assert-type (Option :none) (Option String)) |fallback
```

WASM/WASI 依据相同的 `Bool` 静态条件语义生成代码；已知恒真／恒假常量分支仍先求值条件表达式以保留其中的副作用与检查，对开放类型条件直接拒绝编译。

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

```cirru.no-check
defmacro if-not (condition true-branch & rest-branches)
  if (&> (&list:count rest-branches) 1)
    raise $ str-spaced "|if-not expects (condition then) or (condition then else)" rest-branches
  if (&list:empty? rest-branches)
    quasiquote $ if ~condition &unit $ &let () ~true-branch &unit
    quasiquote $ if ~condition (~ $ &list:nth rest-branches 0) ~true-branch
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
  hint-fn $ {} (:args ([] 'Number))
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

The syntax `macroexpand` only expand syntax tree once:

```
$ calcit eval 'println $ format-to-cirru $ macroexpand $ quote $ let ((a 1) (b 2)) (+ a b)'

&let (a 1)
  let
      b 2
    + a b
```

## 限制

- `if`、`when`、`when-not`、`if-not`、`and` 与 `or` 仅接受 `Bool` 条件，未收窄的非布尔或 `Dynamic` 条件会被拒绝。
- WASM 的 `unsafe-coerce` 只支持保持相同类型、已证明的静态提升（如 UInt32 擦除到 Number）或擦除到 Dynamic，未证明的重新标注报告 `E_WASM_UNSUPPORTED_JS_FFI`。
- WASM 的 `nil?` / `=` 仍要求具体闭合类型以避开开放标量表示碰撞，缺少证据时报告 `E_WASM_NIL_TYPE_EVIDENCE`。
