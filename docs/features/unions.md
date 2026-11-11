---
title: "Unions (defunion)"
summary: "受限的无标签 union：用 defunion 声明“几种已知类型之一”，只能用 match 类型模式拆解"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "defunion"
  - "union"
  - "untagged union"
id: core/features/unions
parent: core/features
---

# Unions (defunion)

`defunion` 给已有的“几种已知类型之一”的数据加一个可检查的闭合边界。union 值在运行时没有包装，`CssValue` 的值就是一个 String 或一个 Number。它不是通用类型能力：不推断、不做谓词窄化、不参与 trait，值必须先用 `match` 的类型模式拆成具体类型才能继续使用。

需要给变体起名字或带多个字段时，用 [Enums](enums.md)。

## 快速示例

- **定义**：`defunion CssValue String Number`
- **标注**：`hint-fn $ Fn CssValue (-> String)`
- **拆解**：`match v ((String s) s) ((Number n) (str n |px))`

## 定义

`defunion` 只能写成具名顶层定义，至少两个成员：

```cirru.no-check
defstruct Point (:x Number) (:y Number)
defenum Shape (:circle Number) (:square Number)

defunion CssValue String Number
defunion Geo Point Shape String
defunion Label String Point
```

成员和 `defstruct` 字段一样直接写类型，带参数的复合类型也不写 quote，例如 `defunion Cells String (List Number)`。旧写法 `'String`、`quote $ List Number` 仍可使用，`calcit edit format` 会改为不带 quote 的写法。表头是小写限定名（如 `app.lib/box`）的复合成员与函数调用写法相同，仍写 `quote $ …`。

成员可以是：

- 内置值类型：`Bool`、`Number`、`String`、`Tag`、`Symbol`、`Buffer`、`CirruQuote`；
- 写全元素类型的容器与函数：`List Number`、`Map Tag String`、`Set Tag`、`Ref Number`、`Fn Number (-> String)`、Tuple；
- 具名 struct、具名 enum；
- core 的 `Option U`，用来表示“这个位置的值可以缺失”，例如可选的子节点；
- 另一个 union，会被展开成它的成员。

Symbol 字面量 `'team-id` 推断为 `Symbol`，所以在 `defunion RouteSegment String Symbol Tag` 下，`[] |team 'team-id :id` 可以直接作为 `List RouteSegment` 传入，不需要运行时解码。

成员按运行时种类区分，每种种类至多出现一次：一个 union 里只能有一个 List（或 Tuple）成员、一个 Map 成员、一个 Fn 成员，struct 和 enum 按定义区分。这样运行时只需浅层判断就能选出唯一成员。

下列成员会报 `E_UNION_MEMBER_INVALID`：

- 任何深度的 `Dynamic`，包括裸 `List`、`Map`、`Fn`，以及 `Map Tag Dynamic`；
- `Nil` / `Unit`、`Never`、数值细化类型；
- 类型变量：第一版 union 不带类型参数；
- 把自己直接列为成员，或者只经过其他 union 绕回自己，例如 `defunion A String A`；
- core 的 `Result`；
- 匿名 enum / struct、trait、`JsObject`；
- 两个运行时种类相同的成员；
- 两个模式名相同的成员，例如来自不同命名空间的同名 struct。

这些规则在 `defunion` 声明处就会检查，即使还没有代码用到这个类型。

schema 里不能内联写 `Union A B`，会报 `E_UNION_INLINE`，必须先用 `defunion` 起名。

## 递归 union

union 可以在成员类型内部引用自己，用来描述树形数据：

```cirru.no-check
defunion Tree Number (List Tree)
defunion Json String Number Bool
  List Json
  Map String Json

defn tree-total (t)
  hint-fn $ Fn Tree (-> Number)
  match t
    (Number n) n
    (List xs) $ foldl xs 0 $ fn (acc x) (&+ acc (tree-total x))
```

- 自引用要放在 `List`、`Map`、`Set` 等容器的元素位置，或者成员 struct 的字段里（`defunion Doc String Section`，`Section` 的 `:children` 是 `List Doc`），这样每一层都有一个具体的值；
- 有期望类型时，`[] 1 ([] 2 3)` 这样的嵌套字面量直接按 `Tree` 检查；
- 运行时判断只看外层种类；受检解码（`parse-cirru-edn-as` 等）会逐层解码，错误给出出错的位置；
- 两个递归 union 之间也能比较：`Map Tag NumericTree` 可以传给 `Map Tag MixedTree`（`NumericTree` 的每个成员都在 `MixedTree` 里），反过来则报类型不匹配；
- 直接把自己当成员（`defunion A String A`）或者只经过其他 union 绕回自己，展开不会终止，报 `E_UNION_MEMBER_INVALID`。

struct 字段同样可以引用自己所在的 struct，例如 `defstruct Node (:data Number) (:children (Map Tag Node))`。

## 用 match 拆解

`match` 的类型模式写成 `(成员名 绑定名)`，最后一个分支可以是 `_`：

```cirru.no-check
defn css-text (v)
  hint-fn $ Fn CssValue (-> String)
  match v
    (String s) s
    (Number n) (str n |px)

defn describe (g)
  hint-fn $ Fn Geo (-> String)
  match g
    (Point p) (str |point: (:x p))
    (Shape s) (str |shape: s)
    _ |text
```

规则：

- 只有被匹配值的静态类型是 union 时才能写类型模式。Dynamic 或未知类型上写类型模式报 `E_UNION_MATCH_PATTERN`；union 上写 Tag 模式也报这个错，两类模式不混用。
- 成员名是内置类型名（`String`、`Number`、`List`、`Map`、`Fn`、`Tuple` 等），或 struct / enum / 嵌套 union 成员的定义名。不在 union 里的名字报 `E_UNION_MATCH_PATTERN`。
- struct / enum 的构造器别名及导入名也可以作为模式，按已有名义定义身份解析。别名与原定义覆盖同一成员，不能各写一个分支；同字段的其他定义不会因此成为成员。
- 分支要覆盖全部成员，否则报 `E_UNION_MATCH_NOT_EXHAUSTIVE`；`_` 可以兜住剩下的成员，所有成员都已覆盖时 `_` 会得到一条不可达警告。
- 分支里的绑定带着成员的具体类型，后续方法调用和字段访问照常检查。

## Option 成员

`Option U` 和其他 enum 成员一样按运行时种类区分，一个 union 至多一个 Option 成员。它适合“几种值之一，或者没有”的变长参数，例如元素函数的子节点：

```cirru.no-check
defunion Node Element Component
defunion Child Node (Option Node)

defn child-text (c)
  hint-fn $ Fn Child (-> String)
  match c
    (Option o) $ .unwrap-or (.map o node-text) |
    _ |present
```

- `match` 用 `(Option o)` 拆出 Option 值，`o` 的类型是 `Option Node`，之后照常 `.map`、`.unwrap-or` 或 `match o (:some x) …`；
- `Option Element` 这类更窄的 Option 可以传给 `Option Node` 成员；
- 普通值不会被自动包成 Option，裸 `nil` 传给 union 位置仍然报类型不匹配，要先写 `nil->option`。

## 值的流向

- 成员类型的值可以传给 union 位置，`|a` 和 `1` 都可以传给 `CssValue`。
- union 可以传给成员更多的 union，`Label`（String、Point）可以传给 `Geo`（Point、Shape、String）。
- union 可以传给 `Dynamic` 位置和无约束的泛型参数，所以可以放进 `List Geo` 再用 `.map` 处理。
- union 可以传给同一个 union 的 JS 可空位置 `JsNullish Geo`，不可变 `Map`/`List`/`Set` 里也一样，例如 `Map Tag Geo` 可以传给 `Map Tag (JsNullish Geo)`；`Ref` 仍然不变。
- union 不能直接传给某个成员类型，也不满足任何 `:where` trait 约束，会报类型不匹配，要先 `match`。
- Dynamic 不能直接传给 union，报 `E_CALL_ARGUMENT_UNPROVEN`，要先经过受检的解码，见下一节。
- 没有期望类型时不推断 union，`if b 1 |a` 的推断结果保持不变。

## 从 Dynamic 解码

`decode-map-as`、`try-decode-map-as`、`parse-cirru-edn-as`、`try-parse-cirru-edn-as` 都接受所有成员均可构建受检数据形状的 union 作为目标类型，结果类型就是这个 union：

```cirru.no-check
defunion Scale Number (List Number)

defn read-scale (props)
  hint-fn $ Fn (Map Tag Dynamic) (-> Scale)
  decode-map-as (.unwrap-or (get props :scale) 1) Scale
```

解码先按运行时种类选出成员，再按该成员完整解码，所以 `[] 1 |a` 会选中 `List Number` 成员并在 `$[1]` 处报错，而不是被当成别的成员。没有种类相符的成员时，错误会列出全部成员，例如 `expected union :Scale (number | list), got string`。 成员内的短类型名及嵌套 union 名使用各自的原始声明命名空间；展平和解码保留完整类型参数，例如 `Cell Number` 的字段及 `Packet Number` 的 payload 都继续检查为 Number。

`decode-map-as` 会把 map 解码成 struct：输入是 map 时，先试 union 里的 `Map` 成员，再依次试 struct 成员。`parse-cirru-edn-as` 的输入里 struct 写成 `%{} :Point …`，按名字选成员。

受检解码要求每个成员都支持数据形状；含 `Fn` 成员的 union 在构建解码器时即报错，不依输入选择跳过不支持的成员。WASM 后端目前不支持 union 目标的解码，和其他受检解码器一样只在 Native 与 JS 上可用。

`Tuple` 成员按 list 种类选中，再检查长度和每个位置的类型，例如 `defunion TripleInput Number $ Tuple Number Number Number` 接受数字或恰好三个数字的 list，两项或四项的 list 返回解码错误。

## 不允许的用法

union 值在 `match` 之前只能原样传递、放进容器或交给泛型函数。下列写法报错：

| 写法                                     | 错误码                   |
| ---------------------------------------- | ------------------------ |
| `string? v`、`number? v` 等类型谓词      | `E_UNION_PREDICATE`      |
| `= v \|a`、`not= v 1` 等与字面量比较     | `E_UNION_LITERAL_EQ`     |
| `str v`、`.method v` 方法调用           | `E_UNION_REQUIRES_MATCH` |
| 传给具体成员类型的参数                   | `W_FN_ARG_TYPE_MISMATCH` |

类型谓词不会让推断收窄，留着只会造成误解，所以直接报错。与字面量的相等比较同理，之后会在类型层面统一限制 equality。

## 运行时表示

`defunion` 求值得到一个 `UnionDef` 值，`type-of` 返回 `:union-def`。union 值本身就是成员的值，没有额外包装，Native 与 JS 后端表现一致。`match` 的类型模式会被预处理成按成员逐个判断的条件分支：内置类型用对应谓词，struct 用 `&struct:matches?` 判断定义，enum 比较 `&enum:definition`。

## 相关文档

- [Enums](enums.md)：带标签的变体
- [Structs](structs.md)
- [Static Analysis](static-analysis.md)
