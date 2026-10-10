# CLI 入口收敛：弃用别名与合并后的入口

关联 #1566。依据 #1566 的 CLI 盘点，维护者批准了不带 `(?)` 的确定项；开放问题（`analyze verify`、`weak-types` 改名、`check-examples` 并入 `test` 等）不在本次范围。

## 决策

- `analyze weak-types --only` 在 match kind 之外接受单个视图 `coverage`、`dynamic-method`、`deprecated-call`。参数解析后把视图改写为原有 analyzer 的选项结构再分发，新入口与旧命令共用同一实现，输出逐字节相同（除耗时）。`check-types` 自己的 `--only none,partial,full` 改名为 `--coverage-level`，避免与 `weak-types --only` 冲突。视图与 match kind 混用、或传入视图无法执行的选项（如 `dynamic-method --ns`）直接报错，不静默忽略。
- `analyze check-public` 改为正确性入口的范围：顶层 `--check-only --ns <ns> [--deps] [--summary-only] [--format]`。仍调用 `public_api_check::run`，并沿用 analyze 路径的安静输出设置，保证 `--entry` 等情况下 stdout 与旧命令一致。`--ns` 不能与 `--keep-going`、`--all-defs`、`--incremental` 或子命令同用；`--deps`、`--summary-only` 要求 `--ns`。
- 结构化 envelope 的 `command` 仍为 `analyze.check-types`、`analyze.check-public` 等旧名。改名会打断现有 JSON 消费者，等旧入口删除时与 #1566 的输出统一一起处理。
- 合并时为 check-types、dynamic-methods、deprecated 与 check-public 补上 EDN，均由同一 JSON envelope 机械转换，不增加字段。
- `analyze quality`、`query pkg`、`query modules`、`tree batch-delete` 只加弃用提示，行为不变。`query config` 的 human 输出增加 `Package` 行以覆盖 `query pkg`；`config modules` 是模块列表的规范入口。事务内的 `batch-delete` 操作名不变，是否保留留给 #1566 的开放问题 6。
- 弃用提示集中在 `cli_handlers/deprecations.rs`：每个旧入口一行 `[Deprecated] ...` 写到 stderr，argh 帮助中的描述也以 `[Deprecated]` 开头。

## 兼容边界

0.29.0 起旧入口继续可用，stdout、退出码与结构化字段不变；0.30.0 删除，删除项与迁移命令写在 `docs/run/upgrade.md`。`config version` 已只输出迁移提示，同样在 0.30.0 删除。Agent 指南的 cursor 章节移到 `docs/run/edit-tree.md`，紧凑 mutation contract 未改，digest 不变。

## 验证

`tests/deprecated_aliases_cli.rs` 对每个旧入口比较新旧 stdout，并断言提示只出现一次且只在 stderr；`cli_handlers::deprecations` 单元测试覆盖视图改写和非法组合。仓库内 CI、脚本和文档改用新入口；calcit.std、js-ffi、respo 的迁移在 PR 中列出，由各仓库单独处理。
