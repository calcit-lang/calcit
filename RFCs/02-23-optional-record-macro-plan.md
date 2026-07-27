# `%{}?` — 可选字段 Record 宏设计

**背景**：`struct` 初始化为 record 时，部分字段在语义上是可选的（`nil` 或缺省），类似 TypeScript 的 `{ a?: number; b?: number }`。当前 `%{}` 宏要求显式传入所有字段。

## 计划新增

### `%{}?`（macro）

初始化 record 时允许省略可选字段，省略的字段默认为 `nil`。

```cirru
; 等价于 TypeScript:  new MyRecord({ x: 1 })  其中 y 为可选
%{}? MyRecord (:x 1)
```

### `&%{}?`（proc）

在已有 record 基础上，允许有选择地更新标记为 optional 的字段，未传入的字段保持原值不变（区别于 `&%{}` 全量替换字段）。

```cirru
; 仅更新 x，y 保持原值
&%{}? my-record (:x 2)
```

## 实现要点

- 需要在 `defrecord` 的字段定义中引入 optional 标记（如 `(:x? :number)` 或 `(? :x :number)`）。
- `%{}?` 展开时跳过未提供的 optional 字段并填 `nil`。
- `&%{}?` 展开时仅替换显式提供的字段，其余字段从原 record 读取。
- 静态类型检查阶段，optional 字段对应类型应自动视为 `:: :optional <type>`。
