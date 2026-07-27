# 2026-07-26 13:13 CLI 长参数提示修复

## 修改概要

- 将 `remote-libs readme` 的过时 `-f` 提示统一为 `--file`。
- 修正 docs、edit、tree 与 markdown 提示中遗留的短参数，使用 CLI 当前公开的 `--filename`、`--context`、`--filter`、`--path`、`--code`、`--file`。

## 验证

- `cargo fmt --all`
- `cargo run --bin cr -- docs remote-libs readme --help`
- `cargo run --bin cr -- docs search --help`
- `cargo run --bin cr -- query search --help`
- `cargo run --bin cr -- tree show --help`
