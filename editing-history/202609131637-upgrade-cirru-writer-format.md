# 升级 Cirru writer 并重新格式化程序 Snapshot

## 背景

`cirru_parser` 0.2.17 将简单叶子的单行长度上限从 16 调整为 48，`cirru_edn` 0.8.2 同步使用该版本。Calcit 同时在普通依赖和构建依赖中直接使用这两个 crate，因此四处声明需要保持一致。

## 修改

- 将普通依赖和构建依赖中的 `cirru_parser` 升级到精确版本 `0.2.17`，并将 `cirru_edn` 升级到 `0.8.2`。
- 以顶层 `:files` 字段识别出仓库内 78 个 Calcit 程序 Snapshot，并使用本次升级后构建的 `calcit edit format` 逐个格式化；其中 77 个文件产生布局变化，1 个文件原本已符合新格式。
- 保留 5 个不含程序 Snapshot 的 Cirru EDN 数据文件不变：质量基线以及 `docs/architectures/` 下的四个架构数据文件。

## 验证

- 对 78 个程序 Snapshot 再运行一轮格式化，前后完整 diff 的 SHA-256 均为 `f3c4ea3ee315fa58b63e8bdb06850389b4e7f160fbb587c143def789c5d649d6`，确认格式化幂等。
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`

