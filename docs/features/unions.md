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

类型名成员和 `defstruct` 字段一样直接写裸符号；带参数的复合类型目前仍写 quote，例如 `defunion Cells String $ quote (List Number)`。旧写法 `'String` 仍可使用，`calcit edit format` 会改为 `String`。

成员可以是：

- 内置值类型：`Bool`、`Number`、`String`、`Tag`、`Symbol`、`Buffer`、`CirruQuote`；
- 写全元素类型的容器与函数：`List Number`、`Map Tag String`、`Set Tag`、`Ref Number`、`Fn Number (-> String)`、Tuple；
- 具名 struct、具名 enum；
- 另一个 union，会被展开成它的成员。

成员按运行时种类区分，每种种类至多出现一次：一个 union 里只能有一个 List（或 Tuple）成员、一个 Map 成员、一个 Fn 成员，struct 和 enum 按定义区分。这样运行时只需浅层判断就能选出唯一成员。

下列成员会报 `E_UNION_MEMBER_INVALID`：

- 任何深度的 `Dynamic`，包括裸 `List`、`Map`、`Fn`，以及 `Map Tag Dynamic`；
- `Nil` / `Unit`、`Never`、数值细化类型；
- 类型变量：第一版 union 不带类型参数，也不能引用自己；
- core 的 `Option` / `Result`：缺失写成 union 外面的 `Option CssValue`；
- 匿名 enum / struct、trait、`JsObject`；
- 两个运行时种类相同的成员；
- 两个模式名相同的成员，例如来自不同命名空间的同名 struct。

这些规则在 `defunion` 声明处就会检查，即使还没有代码用到这个类型。

schema 里不能内联写 `Union A B`，会报 `E_UNION_INLINE`，必须先用 `defunion` 起名。

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

## 值的流向

- 成员类型的值可以传给 union 位置，`|a` 和 `1` 都可以传给 `CssValue`。
- union 可以传给成员更多的 union，`Label`（String、Point）可以传给 `Geo`（Point、Shape、String）。
- union 可以传给 `Dynamic` 位置和无约束的泛型参数，所以可以放进 `List Geo` 再用 `.map` 处理。
- union 不能直接传给某个成员类型，也不满足任何 `:where` trait 约束，会报类型不匹配，要先 `match`。
- Dynamic 不能直接传给 union，报 `E_CALL_ARGUMENT_UNPROVEN`，要先经过受检的解码得到具体类型。
- 没有期望类型时不推断 union，`if b 1 |a` 的推断结果保持不变。

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
