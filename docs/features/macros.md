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

JS 条件统一生成一次求值的真假值表达式；`Bool` 和其他条件都遵守以上规则，不依赖新增 runtime helper。
JS 的 `null`/`undefined` 分别承载 `nil`/`Unit`；真假值判断不是宿主对象的受检解码，不能替代 FFI 类型边界。

WASM/WASI 根据已证明的条件类型生成相同语义：`Number`（包括 0、负零、NaN 和无穷大）、字符串、集合、名义值和函数值是真值；`nil`/`Unit` 是假值；`Bool` 保留运行时分支。
内部推导出的 `Optional Bool` 与具有非零地址表示的可空值（如 `Optional String`）也保留运行时分支，空字符串仍是真值；公开 schema 仍使用名义 `Option` 表达缺席。
已知调用参数通过现有静态调用特化补充类型证据；即使类型已经决定真假，条件中的副作用与错误仍会执行一次。

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

The syntax `macroexpand` only expand syntax tree once:

```
$ calcit eval 'println $ format-to-cirru $ macroexpand $ quote $ let ((a 1) (b 2)) (+ a b)'

&let (a 1)
  let
      b 2
    + a b
```

## 限制

- 当前 WASM 标量 ABI 无法区分未证明的开放条件或可空数字中的 nil 与 0，这些路径报告 `E_WASM_NIL_TYPE_EVIDENCE`。
- WASM 的 `unsafe-coerce` 支持保持相同类型、已证明的静态提升（如 UInt32 擦除到 Number）或擦除到 Dynamic，未证明的重新标注报告 `E_WASM_UNSUPPORTED_JS_FFI`。
- 共同真假值语义不表示 WASM 已支持所有条件中的运算；尚未支持的操作仍按目标边界报错。
