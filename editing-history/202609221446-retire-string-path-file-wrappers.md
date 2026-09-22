# 退役 String-path 文件读写包装

- #1240 的本地生态盘点未发现可执行的 `try-read-file` / `try-write-file` 调用；core 的 `query usages` 也均为零。移除这两项仅作兼容的定义，不改底层 `&fs-read-text` / `&fs-write-text` 或 nominal `FsPath` 的 Result 方法。
- `try-read-dir` 仍由 `FsPath .walk-dir` 使用，本次保留；原始 raising procedures 也暂留在 runtime 边界，后续分别审计。
- 同步更新 WASI 文件架构计划，删除旧包装及 call edges，并把 runtime 占位符标为实际的 data kind；scaffold dry-run 通过，原计划中的 kind 冲突不再存在。
- 文档提供 `fs:path` 加 `.read-text` / `.write-text` 的迁移方式，不建议回退到抛异常的原始文件 procedure。
- 现有 Calcit `fs:path` definition `:tests` 覆盖读成功、读写失败和 Result 类型，故不增加重复 Rust 语义测试。`yarn check-all`、完整 `cargo test --quiet`、`cargo clippy -- -D warnings`、`cargo fmt --all -- --check` 及文档代码块检查（70 文件、340 块）通过。
