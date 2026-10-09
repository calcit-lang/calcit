---
title: "Enums (defenum)"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "defenum"
  - "tagged union"
  - "tagged unions"
id: core/features/enums
parent: core/features
---

# Enums (defenum)

Calcit enums are tagged unions: each variant has a tag and zero or more typed payload values. `defenum` creates an enum definition; `%::` constructs enum values that retain that definition.

## Quick Recipes

- **Define**: `defenum Shape (:circle 'Number) (:rect 'Number 'Number)`
- **Create**: `%:: Shape :circle 5`
- **Match**: `match shape ((:circle r) ...) ((:rect w h) ...)`
- **Type Check**: `assert-type shape 'Shape`

## Defining Enums

```cirru
let
    Color $ defenum Color (:red) (:green) (:blue)
    c $ %:: Color :red
  println c
  ; => $ %:: 'Color :red
```

Variants with payloads:

```cirru
let
    Shape $ defenum Shape (:circle 'Number) (:rect 'Number 'Number)
    c $ %:: Shape :circle 5
    r $ %:: Shape :rect 3 4
  println c
  ; => $ %:: 'Shape :circle 5
  println r
  ; => $ %:: 'Shape :rect 3 4
```

## 带 trait 的 nominal 构造与循环引用

通过 `impl-traits` 为 Enum 附加方法后，构造器的返回 schema 和方法的 `self` schema
可以引用同一个完整 nominal 名称。方法中的 `match self` 可能在 impl 尚未完成预处理时
请求 Enum 定义；编译器应从已有源码恢复变体和 payload 类型，而不是把构造结果退化为
无名的 `Enum`。普通检查、`--check-only --keep-going` 和 `fix --workflow strict --verify`
应遵守同一契约，不需要仅为满足不同检查顺序而增加冗余 `assert-type`。

这种恢复只提供可证明的 nominal 数据形状，不承诺尚未完成的 impl 方法表，也不跳过
变体、payload、泛型约束或方法歧义检查。复现与共享 native/JS 语义测试见
`tests/fixtures/enum-impl-cycle.cirru`；无需新增 Dynamic、unsafe coercion 或平行调用入口。

## Generic Enums

`defenum` accepts an optional generics list right after the type name. Declare generic slots with quoted symbols, then use the applied named type syntax `(:: 'TypeName ...)` in schemas and assertions.

```cirru
let
    ResultX $ defenum ResultX ([] 'T 'E) (:ok 'T) (:err 'E)
    ok $ %:: ResultX :ok 1
    err $ %:: ResultX :err |oops
  assert-type ok $ :: 'ResultX 'Number 'String
  assert-type err $ :: 'ResultX 'Number 'String
  assert= 1 $ &enum:nth ok 1
  assert= |oops $ &enum:nth err 1
```

At the type level, `(:: 'ResultX 'Number 'String)` means the first slot is bound to `T` and the second slot is bound to `E`.

### Generic Enums with `where` Bounds

After the optional generics list, `defenum` may also take a `where` map to constrain type variables with traits. This is useful when enum payloads must support methods like `.debug`.

```cirru
let
    ShownMaybe $ defenum ShownMaybe ([] 'T)
      {} $ 'T Debug
      :some 'T
      :none
    some-val $ %:: ShownMaybe :some 1
    none-val $ %:: ShownMaybe :none
  assert-type some-val $ :: 'ShownMaybe 'Number
  assert= |1 $ match some-val
    (:some item) (&trait-call Debug :debug item)
    (:none) |none
  assert= |none $ match none-val
    (:some item) (&trait-call Debug :debug item)
    (:none) |none
```

The `where` map uses type variables as keys and trait names as values. `%::` checks the variant payloads against those bounds at runtime, so invalid type arguments are rejected when the enum value is created.

## Enums with Struct Payloads

Enum payload slots can reference named structs, not just primitive tags. Use the same applied named type syntax that `defstruct` uses in schemas.

```cirru.no-check
let
    Point $ defstruct Point (:x :number) (:y :number)
    Shape $ defenum Shape (:point Point) (:label :string)
    p $ %{} Point (:x 1) (:y 2)
    shape $ %:: Shape :point p
  assert-type shape Shape
  assert= 1 $ :x (&enum:nth shape 1)
```

Applied generic structs also work in enum payloads:

```cirru
let
    Box $ defstruct Box ([] 'T) (:value 'T)
    Wrapped $ defenum Wrapped ([] 'T)
      :box $ :: 'Box 'T
      :empty
    value $ %:: Wrapped :box
      %{} Box $ :value 1
  assert-type value $ :: 'Wrapped :number
  assert= 1 $ match value
    (:box boxed) (:value boxed)
    (:empty) 0
```

When you annotate an enum payload with a struct type, `%::` validates both the variant tag and the payload value at runtime. Applied struct payload types must use the correct generic arity.

## Creating Instances

Use `%::` with the enum definition, the variant tag, and then the payload values:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :string) (:err :string)
    ok $ %:: ApiResult :ok |success
    err $ %:: ApiResult :err |network-error
  println ok
  println err
```

## Pattern Matching with `match` (recommended)

`match` is a **native syntax** (not a macro) that branches on enum variant tags. Because the compiler sees the full branch structure, it can perform:

- **Exhaustiveness checking** — warns at preprocess time if any variant is not covered
- **Variant arity check** — warns if the binding count doesn't match the variant's payload count
- **Binding type inference** — each binding variable automatically receives the payload type from the enum definition

### Basic usage

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    area $ match c
      (:circle radius) (* radius radius 3.14159)
      (:rect w h) (* w h)
  println area
  ; => 78.53975
```

### Multi-line branch bodies

When a branch body needs more than one expression, indent subsequent lines under the branch:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :string) (:err :string)
    r $ %:: ApiResult :err |network-error
    msg $ match r
      (:ok v) (str-spaced |OK: v)
      (:err e) (str-spaced |Error: e)
  println msg
  ; => Error: network-error
```

### Exhaustiveness checking

If you omit a variant and don't have a wildcard `_` branch, the compiler warns:

```cirru.no-check
match c
  (:circle radius) (* radius radius 3.14159)
  ; "⚠" Warning: match on "`Shape`" is not exhaustive. Missing variant (s) : [:rect]
```

The check fires only when the compiler can infer the enum type of the matched value — for example, when the value is directly constructed with `%::`, or when the function parameter is annotated with an enum type in its schema. When the type cannot be inferred at preprocess time, the match still works at runtime but no compile-time warning is issued.

Use `_` as a wildcard to catch remaining variants:

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    label $ match c
      (:circle _r) |round
      _ |other
  println label
  ; => round
```

### 字面量模式

`match` 的分支也可以直接比较普通 tag、字符串、数字、`true`/`false` 和 `nil`，用于对非 enum 的值分派。字面量模式写成不带括号的值，`(:tag ...)` 仍然表示 enum 变体：

```cirru.no-check
match action
  :mount $ setup! target
  :before-update $ cleanup! target
  _ &unit
```

- 匹配按值相等，从上到下取第一个命中的分支，被匹配的值只求值一次。
- 通配符 `_` 只能放在最后；没有 `_` 且无分支命中时，运行时报错 `match: no matching branch for literal value: <value>`。
- 同一个 `match` 不能混用字面量模式和 enum 模式。
- 预处理阶段对重复的字面量、与被匹配值已知类型不符的字面量（例如 `Option` 值写了 `:some` 而不是 `(:some)`）给出 `[Warn]`。
- tag、字符串这类开放集合不做穷尽性检查；需要穷尽性时定义 enum。

`case` 与 `case-default` 在所有模式都是字面量时展开为这种 `match`（`case-default` 的默认值成为末尾的 `_` 分支）。`case-default` 已标记 `:deprecated`，请改写为 `match` 加 `_` 分支；模式是表达式或变量的旧写法仍由 `&case` 处理，需要时改用 `cond`。

### No-match runtime error

If no branch matches at runtime (and no `_` wildcard is present), `match` throws:

```
match: no matching branch for tag :unknown-tag
```

这是明确的运行时错误，不会静默返回 `nil`。

### 从旧语法迁移

Calcit 0.14.16 已移除旧的 `tag-match` macro，具名与匿名 enum 都统一使用原生 `match`。分支 AST 保持一致，升级前请固定使用 Calcit 0.14.15 执行：

```bash
calcit calcit.cirru fix --rule tag-match-to-match-v1
```

review 并应用迁移后，运行受影响的 Calcit `:tests` 与 JS/WASM build；再切换到 0.14.16 或更新版本。编译器会继续报告未覆盖的 variant，可补充分支或显式添加 `_` 分支。

## Zero-payload Variants

When a variant has no payload, the pattern is just the tag wrapped in parentheses:

```cirru
let
    MaybeInt $ defenum MaybeInt (:some :number) (:none)
    some-val $ %:: MaybeInt :some 42
    none-val $ %:: MaybeInt :none
    extracted $ match some-val
      (:some v) (* v 2)
      (:none) nil
  println extracted
  ; => 84
```

## Checking Enum Origin

Use the Option-returning `enum-definition` API to inspect the definition behind an enum value:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :number) (:err :string)
    x $ %:: ApiResult :ok 1
  println $ match (enum-definition x)
    (:some original) $ = original ApiResult
    (:none) false
  ; => true
```

## 按运行时类型分派：`data-view`

`data-view` 把一个 Dynamic 值转换成封闭的 `Data` enum，之后用 `match` 分派，分支里的 payload 带有类型：

```cirru
defn preview (x)
  match (data-view x)
    (:nil) |nil
    (:string s) $ to-lispy-string s
    (:number n) $ str n
    (:list xs) $ str |List/ (count xs)
    (:map m) $ str |Map/ (count m)
    _ |other
```

`Data` 的变体是 `:nil` `:bool` `:number` `:string` `:tag` `:symbol` `:list` `:map` `:set` `:fn` `:enum` `:struct` `:ref` `:other`。标量分支提供对应的具体类型；`:list` / `:map` / `:set` 的 payload 是 `List<Dynamic>` / `Map<Dynamic,Dynamic>` / `Set<Dynamic>`，转换不会深入元素。集合元素以及 `:fn` / `:enum` / `:struct` / `:ref` / `:other` 的 payload 仍是 Dynamic，具体使用前需要进一步 decode 或类型证明。

`:other` 保留不属于前面类别的值，包括 Buffer、类型定义和后端内部或宿主值。二选一的判断仍然使用 `string?` 等谓词。`data-view` 是普通 core 函数，通过现有谓词构造 Data。`match (data-view x)` 直接写在 `match` 里时，预处理会把它降级成只测试有分支的变体的谓词链，不构造 Data 值；分支绑定的类型与 Data 的 payload 声明一致。把 `data-view` 的结果存进变量再 `match` 时，仍然构造 Data 值。

### 与 `type-of` 的关系

`type-of` 返回无类型的 tag，分支里的值不会被收窄，各后端返回的 tag 集合也不完全一致。需要按运行时类型分派时使用 `match (data-view x)`；二选一的判断使用 `string?` 等谓词。`type-of` 保留为低层原语，供 core 内部和调试输出使用。

## Common Patterns

### Result / Either type

```cirru
let
    AppResult $ defenum AppResult (:ok :number) (:err :string)
    compute $ fn (x)
      if (> x 0)
        %:: AppResult :ok $ * x 10
        %:: AppResult :err |negative-input
    handle $ fn (r)
      match r
        (:ok v) (str-spaced |result: v)
        (:err e) (str-spaced |failed: e)
  println $ handle (compute 5)
  ; => result: 50
  println $ handle (compute -1)
  ; => failed: negative-input
```

### Compose enums with functions

```cirru
let
    Status $ defenum Status (:pending) (:done :string) (:failed :string)
    pending $ %:: Status :pending
    done $ %:: Status :done |ok
    is-done $ fn (s)
      match s
        (:done _) true
        (:pending) false
        (:failed _) false
  println $ is-done pending
  ; => false
  println $ is-done done
  ; => true
```

## Type Annotations

Field types in `defenum` declarations participate in type checking:

```cirru.no-run
; (:ok :string) means the :ok variant has one :string payload

defenum ApiResult (:ok :string) (:err :string)

; (:point :number :number) means :point has two :number payloads

defenum Shape (:point :number :number) (:circle :number)

; generic struct payloads use applied named types

defstruct Box ([] 'T) (:value 'T)

defenum Wrapped ([] 'T)
  :box $ :: 'Box 'T
  :empty

; (:none) means no payload

defenum MaybeInt (:some :number) (:none)
```

Payloads may also reference named struct types (illustrative, cross-file type ref):

```cirru.no-check
; defstruct and defenum from the same snippet context:

defstruct Point (:x :number) (:y :number)

defenum Shape2 (:point Point) (:circle :number)
```

Runtime type validation is enforced at instance creation — passing the wrong type to `%::` will raise an error.

## Automatic Anonymous-to-Named Enum Rewrite

When a function parameter is typed as an enum, the preprocessor can rewrite an anonymous enum (`%:: _`) to the expected named definition.

```cirru
let
    Result0 $ defenum Result0 (:err :string) (:ok)
    takes-result $ fn (r)
      :: :fn $ {} (:return :dynamic)
        :args $ [] 'app.main/Result0
      match r
        (:ok) :ok
        (:err msg) msg
        _ :unknown
  ; The anonymous enum is rewritten to the expected named enum:
  assert= :ok $ takes-result (%:: _ :ok)
  ; Equivalent to:
  assert= :ok $ takes-result (%:: Result0 :ok)
```

Requirements for the rewrite to trigger:

- The function schema must reference a named enum type such as `'ns/EnumName`
- The argument at the call site must be an anonymous enum literal (`%:: _`)
- The enum definition must be resolvable at preprocess time

If any condition is not met, the argument is left unchanged (no error is raised). This makes the rewrite safe to use alongside existing code.

## Notes

- Enum values are immutable and retain their enum definition.
- `match` 是唯一的 enum 模式匹配表层语法，并提供穷尽性检查。
- Use `&enum:nth` to directly access payload values by index (0 = tag, 1+ = payloads).
- `%:: _ :tag ...` constructs an anonymous enum; `%:: EnumDef :tag ...` constructs a named enum.

## See Also

- [Anonymous enums](anonymous-enums.md) — short-lived enum values without `defenum`
- [Structs](structs.md) — named-field structures with `defstruct`
- [Static Analysis](static-analysis.md) — type checking for enum payloads and type slots
