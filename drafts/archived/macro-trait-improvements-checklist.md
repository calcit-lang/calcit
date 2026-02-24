# Macro & Trait 改进清单（Draft）

更新时间：2026-02-13

## 目标

- 提升 `.method` 与 `&trait-call` 的使用边界清晰度。
- 降低 macro 展开与 trait 分发的排障成本。
- 保持现有行为兼容，优先增加诊断与可观测性。

## P1（本轮开始）

- [x] `.method` 同名方法冲突告警（建议改用 `&trait-call` 消歧义）
  - 场景：receiver 存在多个带 trait-origin 的 impl，且都包含同名方法。
  - 目标：preprocess 阶段提示“当前命中依赖优先级”，建议显式 trait 调用。
- [x] 顶层约束告警
  - 场景：`impl-traits` 出现在 `defn/defmacro` 内部，导致 preprocess 难以内联。
  - 目标：给出明确 warning，提示迁移到顶层 `def`。
- [x] `macroexpand` 诊断链路
  - 记录展开路径（macro A -> macro B -> ...），提升报错可读性。

## P2

- [x] trait default 实现 identity 增强（不仅 Some/None）
- [x] `quasiquote` 单遍替换优化

## P3

- [x] hygienic 宏辅助层（在 `gensym` 之上提供更易用抽象）

## 本轮执行记录

- [x] 新建改进清单
- [x] 开始并完成：`.method` 同名方法冲突告警
- [x] 开始并完成：`impl-traits` 顶层约束告警（仅在 `--warn-dyn-method` 模式提示）
- [x] 开始并完成：`macroexpand` 诊断链路（输出展开路径）
- [x] 开始并完成：trait default 实现 identity 增强（优先 def_ref，比 Some/None 更精确）
- [x] 开始并完成：`quasiquote` 单遍替换优化（移除预扫描，合并为单次递归）
- [x] 开始并完成：hygienic 宏辅助层（新增 `with-gensyms` 与示例测试）
