# 2026-05-26 match 文档与 check-md 校验

## 修改内容

- 将 `match` 的文档说明统一为同时适用于 enum tuple 和 plain tuple，不再把 `tag-match` 描述成 plain tuple 的唯一入口。
- 更新 `features/enums.md`、`features/tuples.md`、`features/common-patterns.md`、`features.md`、`quick-reference.md`、`run/agent-advanced.md` 中的推荐表述与示例。
- 为 `agent-advanced.md` 里无法独立运行的 DOM / 事件示意片段补上 `cirru.no-check`，使 `cr docs check-md` 只校验可执行代码块。

## 验证

- `cargo run --bin cr -- eval $'match (:: :point 10 20)\n  (:point x y) (+ x y)\n  _ 0'`
- `cargo run --bin cr -- docs check-md docs/features.md`
- `cargo run --bin cr -- docs check-md docs/features/common-patterns.md`
- `cargo run --bin cr -- docs check-md docs/features/enums.md`
- `cargo run --bin cr -- docs check-md docs/features/error-handling.md`
- `cargo run --bin cr -- docs check-md docs/features/traits.md`
- `cargo run --bin cr -- docs check-md docs/features/tuples.md`
- `cargo run --bin cr -- docs check-md docs/quick-reference.md`
- `cargo run --bin cr -- docs check-md docs/run/agent-advanced.md`

## 知识点

- 原生 `match` 可以直接匹配 plain tuple；它不只适用于带 `defenum` 类型信息的 tuple。
- 文档里的示意片段如果依赖 DOM helper、事件桥接函数或其他未加载上下文，应优先标记为 `cirru.no-check`，否则会被 `check-md` 的 `eval` 阻断。