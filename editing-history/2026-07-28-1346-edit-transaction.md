# `cr edit transaction` 第一版

## 概要

- 在既有 `cr edit` 命令组中增加 `transaction`，输入复用现有 `edit`、`tree`、`config` 参数序列。
- 以 Cirru EDN argument lists 为主输入，并允许在 `--code` 后直接嵌入 quoted AST；JSON 作为兼容格式保留。支持 `--dry-run`、`--expect-revision` 和 human/JSON 输出。
- 所有 operation 只修改同目录 staged snapshot；全部成功、snapshot 可重新加载/序列化且原文件 revision 未变化后，才通过 rename 一次提交。
- staged 文件保留原 snapshot 权限；失败、stale revision 与 dry-run 都由 guard 清理，不修改原文件。
- transaction 会捕获每个子命令的 stdout/stderr，保证成功时 JSON stdout 仍是单个可解析值。

## 关键取舍

- 没有立即把全部 handler 重构成新的内存 mutation API，因为这会同时改变大量稳定的 `edit/tree/config` 路径。
- 第一版通过 staged snapshot 调用当前 `cr` 子命令，直接复用参数解析、namespace 边界、tree path、schema 和保存校验。
- transaction 仅允许 `edit`、`tree`、`config`，拒绝嵌套 transaction 和只读/执行类命令。
- snapshot revision 当前是原文件内容的 MD5 opaque ID；执行前和最终 rename 前都会检查，避免 stale write。

## 验证

- 新增 JSON/Cirru 解析、Cirru EDN 内嵌 quoted code、非法 command、nested transaction、stale revision、dry-run、operation failure rollback、成功提交测试。
- 真实临时 snapshot 验证了 `config version`、`edit doc` 与 `tree replace` 的组合提交及失败回滚。
- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`（新增 staged transaction 场景，12/12）
- `yarn check-all`
