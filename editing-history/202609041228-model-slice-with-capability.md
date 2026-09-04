# Model `slice` with a capability / 用能力约束建模 `slice`

## Context / 背景

Review found that independent `C`, `K`, and `T` variables do not express the element/key relationships of heterogeneous collection helpers. A bare `C -> C` signature also admits receivers that the runtime cannot slice.

Review 指出，彼此独立的 `C`、`K`、`T` 无法表达异构集合 helper 的元素与键关系；单纯的 `C -> C` 还会错误接受运行时不可切片的 receiver。

## Change / 修改

- Withdraw the unsound `filter` and `update` schema migrations until the type system can represent their container/member relationships.
- Add a nominal `Sliceable` trait implemented by built-in `List` and `String` values.
- Keep `slice` as `C -> C`, but require `C: Sliceable` so return identity and receiver capability are both enforced.
- Add a negative preprocessing fixture proving that `slice 1 0 1` reports `W_GENERIC_WHERE_BOUND_MISMATCH`.
- Regenerate the bundled-core quality baseline and Dynamic classification. The final branch removes two public-core Dynamic positions (280 -> 278) without introducing new open internal schemas.

- 撤回无法正确表达关系的 `filter` 与 `update` schema 迁移，等待类型系统具备容器/成员关联能力后再推进。
- 新增名义化 `Sliceable` trait，并由内建 `List` 与 `String` 实现。
- `slice` 保留 `C -> C`，同时要求 `C: Sliceable`，共同约束返回同型与 receiver 能力。
- 新增负向预处理 fixture，验证 `slice 1 0 1` 会报告 `W_GENERIC_WHERE_BOUND_MISMATCH`。
- 重生成 core quality baseline 与 Dynamic classification；最终分支在不增加内部开放 schema 的前提下移除两个 public-core Dynamic 位置（280 -> 278）。
