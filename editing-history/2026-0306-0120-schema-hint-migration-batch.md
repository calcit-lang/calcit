# 2026-03-06 01:20 Schema Hint Migration Batch

## 概要

本次集中推进 `hint-fn` 新 schema map 语法迁移，覆盖 core 与测试快照中的多批函数/示例；并持续以 `cargo run --bin cr -- calcit/test.cirru -1` 与 `yarn check-all` 做回归验证。

## 关键变更点

- 在 `src/cirru/calcit-core.cirru` 中将大量旧写法 `hint-fn $ return-type ...` 迁移为：
  - `hint-fn $ {}`
  - `:args` / `:rest` / `:return` / `:generics` 等字段显式表达
- 对已由 `:args`/`:rest` 覆盖的参数类型，移除冗余 `assert-type`（保持必要运行时校验逻辑不变）。
- 同步修正若干 schema 与实现不一致处（例如部分参数应为 `:number`/`:fn` 的场景）。
- 完成多处嵌套局部函数（如 `%map`、`%map-indexed`、`%pairs-map`、`%select-keys`、`%repeat` 等）的 hint 迁移。
- 继续保持 `calcit/*.cirru` 测试快照侧的新语法一致性。

## 工具与流程

- 使用新增命令 `cr edit format` 对变更的 snapshot 文件做规范化重写：
  - `src/cirru/calcit-core.cirru`
  - `calcit/test-generics.cirru`
  - `calcit/test-js.cirru`
  - `calcit/test-types-inference.cirru`
  - `calcit/test-types.cirru`
- 使用 `cargo fmt` 统一 Rust 代码格式。

## 验证

- 多轮执行并通过：
  - `cargo run --bin cr -- calcit/test.cirru -1`
  - `yarn check-all`
- 迁移过程中持续统计 `src/cirru/calcit-core.cirru` 内剩余旧模式数量，确认总体下降趋势。

## 经验

- 在 schema-first 迁移中，优先迁移“签名简单、类型清晰”的函数可快速稳定推进。
- 对复杂逻辑函数保留运行时断言，但避免与 `:args` 重复声明同一层参数类型。
- 每批次后立即执行 targeted + full 验证，能快速定位回归并降低累计风险。
