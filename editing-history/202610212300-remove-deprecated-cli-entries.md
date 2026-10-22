# 0.31.0 删除已弃用的 CLI 入口

## 决策与范围

#2033 的任务是在 0.31.0 删除 #1566 合并时留下的旧 CLI 入口。这次删除了三批入口：0.29.0 与 0.30.0 弃用的叶子、0.29.2 弃用的 `tree insert/swap` 与 `cursor` 旧叶子、`query host-procs`。同时删除了 `query def`、`query schema`、`tree show` 的 `--json` 开关。图命令（`analyze call-graph`、`effects-graph`）遇到未知的 `--format` 值时，此前会回退到文本输出，现在改为直接报错。

argh 无法隐藏 subcommand 变体，所以删除一个入口就要删除它的 variant。为了不让用户只看到 argh 的 `Unrecognized argument`，`cr.rs` 在 argh 解析之前先扫描原始参数。它只检查第一个命令组 token 和紧随其后的叶子：命中删除表时，输出一行报错，写明替代写法和 `docs/run/upgrade.md`。命中的命令不会写 Snapshot 或 cursor。`edit transaction` 中的旧操作在执行前校验，整个事务失败并报出操作序号。

原本旧叶子被归一化为新入口；现在改为内部的 `AnalyzeView` 和 `CursorAction` 枚举，`tree insert --at` 与 `tree swap --with` 直接分派。`analyze weak-types --only deprecated-call|dynamic-method` 与 `analyze call-graph --counts|--diff` 仍复用原实现，envelope 的 `command` 名（如 `analyze.deprecated`）保持不变。

## 保留的入口

以下入口仍保留，并继续在 stderr 输出 `[Deprecated]` 提示：

- `analyze check-types`、`analyze check-public`、`analyze quality`：#1566 的盘点显示 calcit.std、js-ffi、respo 的 CI 或脚本仍在调用。下游尚未确认迁移，现在删除会直接打断它们的 CI。
- `ffi export --json`：calcit.std 的 FFI inventory 流程和本仓库文档仍以它为入口。
- `--strict-types`、`--compat-types`：属于全局选项，不是子命令入口；`scripts/check-strict-default.mjs` 与 `scripts/check-strict-nil-ecosystem.mjs` 仍依赖它们。

删除这些入口需要先在下游仓库完成迁移，再单独开 PR。

## 兼容边界

- `cursor anchor`、`cursor marks`、`cursor clipboard` 的无动作写法不是旧叶子，保持可用。
- `cursor apply` 的操作名（`insert-before`、`swap-next` 等）与删除的 tree 叶子无关，保持不变。
- `--format text|tree` 在图命令上仍等价于 `human`。
- `query search-expr --json` 描述的是输入编码，不受影响。
- RFC 与历史记录中的旧写法属于历史档案，不改写。

## 验证方式

- `tests/deprecated_aliases_cli.rs` 改为两类断言。被删除的入口：退出码非 0、stdout 为空、stderr 写明替代写法、Snapshot 与 cursor 状态不变。替代写法：直接运行并检查结果。
- 保留的别名继续与替代写法逐字节比较。
- `tests/format_flag_cli.rs` 覆盖已删除的 `--json` 开关、未知图格式报错，以及 `ffi export --json` 的弃用提示。
- `scripts/check-agent-interface.mjs` 改用 `--format json`，并断言 `query def --json` 的删除报错。
- 规范化模块的单元测试覆盖删除表的正反例（包括 `ffi export --json` 与 `search-expr --json` 不被误判）。
