# 0.14.19 发版准备清理

## 中文

- 把 `tag-match` 与旧 Struct 迁移桥的文档及 CLI 诊断边界从错误的 0.15 更正为实际发布边界 0.14.15 → 0.14.16+。
- 收紧 crates.io 包清单，排除 RFC、仓库维护配置、profiling、历史记录与 JavaScript 工具链文件，同时保留编译所需源码、测试、schema、README 和内嵌 Agent 文档。
- 删除两个从未启用、由 `allow(dead_code)` 掩盖的字符串匹配 helper。
- 通过 `cargo package --list`、`cargo package` 与 npm dry-run 核对发布内容，避免把仓库内历史文件误当成需要删除的产品代码。

## English

- Correct the documented and CLI-diagnosed `tag-match` and legacy Struct migration boundary from the inaccurate 0.15 wording to the shipped 0.14.15 → 0.14.16+ transition.
- Tighten the crates.io package contents by excluding RFCs, repository maintenance configuration, profiling, history, and JavaScript tooling while retaining required source, tests, schemas, README, and the embedded Agent guide.
- Remove two never-enabled string matching helpers hidden behind `allow(dead_code)`.
- Verify release contents with `cargo package --list`, `cargo package`, and an npm dry run instead of treating repository history as removable product code.
