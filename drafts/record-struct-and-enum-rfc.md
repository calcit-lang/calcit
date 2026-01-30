# RFC: Record Struct 重构与 Enum 语法

## 背景与问题

当前 record 既承担“数据结构”又承载“类型/结构描述”的角色，且 enum 仍通过 record 作为临时表示。这会带来：

- record 的语义混用（值/类型/原型）
- enum 在语法层没有直接定义方式
- 类型标注表达能力不足，难以描述“结构形状”

## 目标

- 允许用“结构体”语法显式定义 record 的形状与字段（类型可标注或大量使用 :dynamic）。
- 为 enum 引入独立语法，替代“用 record 表示 enum”的临时用法。
- 保持现有运行时数据结构与语义尽可能稳定，改动聚焦在语法与类型信息上。

## 非目标

- 不改变现有 record/tuple 的运行时行为。
- 不引入复杂类型推断（仍以显式标注为主）。

## 新语法提案

### 1) record struct 定义

新增结构定义语法（以宏形式提供，内部使用 `&struct::new`）：

```
defstruct Person
  :name :string
  :age :number
  :position :dynamic
```

- 字段类型支持内置 tag（:string、:number 等）。
- 未标注或显式使用 :dynamic 视为动态。
- 标注为 nil 时自动视为 :dynamic。
- 结构定义仅用于类型与结构信息，不必等同于“record 值”。

使用 record 结构示例：

```
defn make-person (name age position)
  let
      p $ %{} Person (:name name) (:age age) (:position position)
    assert-type p Person
    .rename p |NewName

defn update-person (p)
  assert-type p Person
  &record:assoc p :age 30

defn bind-class (p)
  &record:with-class p BirdClass
```

### 2) enum 定义

新增 enum 语法（以宏形式提供，内部使用 `&enum::new`）：

```
defenum Result
  :ok
  :err :string
```

使用 tuple 示例（废弃 %%::，使用 %:: 表示 enum tuple）：

```
defn make-ok (value)
  %:: Result :ok value

defn make-err (message)
  %:: Result :err message

defn attach-class (t)
  &tuple:with-class t ActionClass
```

## 类型标注连接

- 在类型标注表达中，允许引用 defstruct/defenum 的名称。
- 例如：

```
assert-type user Person
assert-type result Result
```

## 迁移策略

- 保留旧的 record-as-enum 解析路径一段时间（兼容期）。
- 逐步将标准库与测试用例迁移到 defenum 语法。

## 迁移指南

### 1) Enum tuple 构造

旧写法（已移除）：

```
%[] Result :ok value
```

新写法（唯一可用）：

```
%:: Result :ok value
```

### 2) 绑定 class

旧写法（已废弃）：

```
%%:: Result :ok value ActionClass
```

新写法：

```
&tuple:with-class (%:: Result :ok value) ActionClass
```

### 3) 运行时校验

`%::` 会校验变体参数数量，确保与 enum 定义一致；不匹配会抛错。迁移时请确认所有变体调用的 payload 数量与定义一致。

### 4) defstruct/defenum 的参数形式

defstruct/defenum 现在是宏，不是语法。字段与变体使用 list 形式传参（等价于 Cirru 解析成 list）。

```
defstruct Person
  :name :string
  :age :number

defenum Result
  :ok
  :err :string
```

## 风险与兼容性

- 需谨慎处理“record 作为值”的现有用法。
- 新语法应避免与已有宏/语法冲突。

## 公开问题

- defstruct/defenum 的最终命名。
- defstruct 是否生成运行时 record 原型，还是仅类型信息。
- 与现有 `defrecord!` 的关系与迁移策略。
