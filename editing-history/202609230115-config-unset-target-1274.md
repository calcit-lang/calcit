# 支持通过 calcit config 撤销 entry target（#1274）

## 背景

`calcit config set target browser` 会写入 entry 的 `:target`，但没有结构化命令恢复“未指定 target”。
`null`、空值都会被当作非法 target 拒绝。Respo 这类同时用于 browser 与 Node SSR 的项目只能手改
Snapshot 回退。

## 改动

- `src/cli_args.rs`：新增 `calcit config unset <key> [--entry <name>]` 子命令。
- `src/bin/cli_handlers/config.rs`：`handle_unset` 目前只接受 `target`，把 entry 的 `:target` 删除，
  恢复未指定语义；重复 unset 返回稳定提示且不改变 Snapshot 内容；未知 key 明确报错且不写入。
  `Unset` 归入会写回 Snapshot 的 mutation，因此沿用同一套工具链版本门禁。
- `src/bin/cli_handlers/command_echo.rs`：补 `unset` 的 command echo。
- 文档：`docs/run/entries.md` 与 `docs/run/upgrade.md` 记录用法、幂等性与迁移建议。

## 测试

- 单测 `config_unset_target_round_trips_and_is_idempotent`：覆盖 set→unset 后 `:target = None`、
  重复 unset 的 Snapshot 字节不变、未知 key 拒绝且不写入。
- 解析测试 `parses_config_unset_target`：覆盖默认 entry 与 `--entry server` 两种形式。
- 真实项目：在 Respo 副本上执行 set browser → unset → `--check-only` 与 `yarn test-dom-host`
  的 Node SSR smoke 均通过。

## 验证

`cargo fmt`、`cargo clippy -- -D warnings`、`cargo test --release` 全绿；`bash scripts/check-docs-md.sh`
70 files / 340 blocks 通过；`yarn check-strict-default` 通过。
