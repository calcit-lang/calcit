# 泛型 `:where` 约束 MFS

日期：2026-05-31

状态：Draft

## 目标

在现有函数 schema 泛型链路上，增加最小可用的 trait 约束能力：

- schema / `hint-fn` 可以声明 `:where`；
- 调用点在泛型变量完成绑定后，检查绑定结果是否满足 trait 约束；
- 先走主链路，不引入完整 trait solver，不处理复杂的 transitive requires 推导。

## 当前实现边界

本轮只覆盖：

- `CalcitFnTypeAnnotation` 的 `:where` 解析与序列化；
- 顶层 `:schema` 注入 `hint-fn` 后，用户函数调用的 `:where` 检查；
- 局部函数 / 期望函数类型场景的 `:where` 检查；
- trait 约束当前以 preprocess warning 形式暴露。

本轮不覆盖：

- trait `requires` 的递归满足性；
- 约束失败直接升级为 hard error；
- 基于 `:where` 的代码生成优化；
- 复杂多态分派或 impl 搜索策略调整。

## Canonical Cirru EDN 形态

### 顶层 wrapped schema

`CodeEntry.:schema` 在 snapshot / Cirru EDN 里的推荐形态：

```cirru
%{} :CodeEntry
  :doc |demo
  :code $ quote
    defn show-it (x)
      .show x
  :examples $ []
  :schema $ :: :fn
    {}
      :generics $ [] 'T
      :where $ {}
        'T Show
      :args $ [] 'T
      :return :string
```

### 多 trait 约束

单个类型变量绑定多个 trait 时，值是一个列表：

```cirru
:: :fn $ {}
  :generics $ [] 'T 'U
  :where $ {}
    'T $ [] Show Eq
    'U Ord
  :args $ [] 'T 'U
  :return 'T
```

### 一行 Cirru EDN 示例

```cirru
(:: :fn ({} (:generics ([] 'T 'U)) (:where ({} ('T ([] Show Eq)) ('U Ord))) (:args ([] 'T 'U)) (:return 'T)))
```

## 约束格式结论

`:`where` 当前使用 map，而不是 list-of-tuples。这一点刻意对齐 Rust `where` 的分离式设计：泛型名字继续放在 `:generics`，trait 约束单独放在 `:where`，避免把“变量声明”和“约束条件”混在同一个位置里。

- key: 泛型变量，例如 `'T`
- value: 单个 trait，或 `[]` 列表中的多个 trait

也就是：

```cirru
:where $ {}
  'T Show
  'U $ [] Eq Ord
```

不要写成旧草案里的 tuple 形式。`:: 'Show 'T` / `:: 'Eq 'U` 这类写法已经废弃，不应再出现在任何新文档、示例或测试输入里。

后者不要再作为新文档示例。

在概念上可以把它理解成 Rust 里的：

- `fn f<T, U>(...) where T: Show + Eq, U: Ord`
- 在 Calcit 中对应为 `:generics $ [] 'T 'U` 与 `:where $ {} 'T $ [] Show Eq  'U Ord`

## 与 EDN / 内部表示的关系

- Cirru 文本里写 `'T`；
- 进入 EDN 后，它仍然是 symbol，只是内部不会把前导 `'` 作为名字的一部分存储；
- trait 名称在 schema 里直接写定义名，例如 `Show`、`Eq`，不加 `:` 前缀。

## 主链路开发步骤

1. 文档先固定 canonical `:where` 形态，避免继续沿用旧 tuple 草案。
2. schema parse / serialize 支持 `:where`。
3. 用户函数与局部函数调用，在泛型绑定后做 trait 约束检查。
4. 增加 parse / roundtrip / warning 级别测试。
5. 视结果决定是否把 warning 升级成 error，并是否纳入 trait `requires`。

## 当前测试最小集

至少保留以下方向：

- `hint-fn` 里的 `:where` 能被提取；
- 顶层 schema roundtrip 后 `:where` 不丢失；
- 满足 trait 约束时无 warning；
- 不满足 trait 约束时出现明确 warning。

## 下一步

- 先把 `03-05-function-schema-dual-track-rfc.md` 中旧的 `:where` tuple 示例替换掉；
- 再补 schema roundtrip 测试，确保文档与实际输出一致；
- 之后再决定是否把约束失败升级为 `E_GENERIC_WHERE_BOUND_MISMATCH`。