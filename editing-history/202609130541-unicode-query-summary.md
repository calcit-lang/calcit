# Unicode 安全的 query 摘要

## 背景

`query defs` 等人类可读查询按 UTF-8 字节位置截断文本。截断点落入中文字符内部时，Rust String slice 会 panic，导致用户与 Agent 都无法取得定义清单。

## 变更

- 统一按 Unicode 字符数量定位摘要前缀，不再使用未经边界检查的字节索引。
- `query defs` 继续保持单行摘要和 ASCII `...` 格式。
- 同时修正 query definition expression、schema preview、symbol context 与节点预览中的同类截断路径，避免相同 panic 转移到相邻命令。
- Agent CLI 集成回归直接运行包含中文文档的 WASI fixture，验证完整中文摘要与长摘要省略号格式。

## 验证

- `calcit/test-wasi-command.cirru query defs app.main` 不再 panic，并保留中文文档。
- `src/cirru/calcit-core.cirru query defs calcit.core` 完整执行成功。
- `yarn check-agent-interface`：32/32 machine scenarios 及 Unicode human-output regression 通过。
