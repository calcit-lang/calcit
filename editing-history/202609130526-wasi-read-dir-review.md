# WASI 目录枚举 review 修正

## 背景

PR #1042 的 review 指出平台文档、`FsPath` tag lowering 与目录结果内存容量存在三个边界缺口。目录 adapter 虽限制条目数和路径总字节数，但此前没有在动态分配前统一保证线性内存容量。

## 变更

- 文档明确生成的 JavaScript 文件枚举只适用于 Node host，browser 不支持 `.read-dir` 与 `.walk-dir`。
- WASM lowering 从 `&fs-read-dir` 的 `FsPath` struct reference 参数解析 tag，避免依赖字符串查表及其潜在 panic。
- 在读取目录前按公开上限预留 list、buffer、带 padding 的路径 String、每项 `FsPath` 和成功 Result 所需容量。
- 地址空间溢出或 `memory.grow` 失败时关闭目录 descriptor 并返回失败状态，由公开边界保留为 `Result :err`。
- 确定性 Preview 1 host 回归检查目录 adapter 已扩展到有界分配所需的线性内存。

## 验证

- `cargo fmt --check`
- `cargo test --lib`
- `bash scripts/test-wasi-preprocess.sh`
- `yarn check-all`
