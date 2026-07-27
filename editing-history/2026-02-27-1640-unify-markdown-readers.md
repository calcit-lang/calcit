# 2026-02-27 16:40

## 知识点

- 将 `docs read` / `docs agents` / `libs readme` 的 Markdown 读取行为统一到共享模块。
- 共享能力包括：标题提取、章节匹配、`--full`、`--with-lines`、`--no-subheadings`、提示输出与无匹配错误处理。
- `docs agents` 增加本地缓存刷新路径与长度展示，并复用统一渲染逻辑。
- `libs readme` 对齐结构化读取参数，支持本地与远程 README 的一致体验。

## 改动概要

- 新增 `src/bin/cli_handlers/markdown_read.rs`，沉淀可复用的 markdown section 渲染流程。
- `src/bin/cli_handlers/docs.rs` 切换为共享渲染入口，并精简参数传递结构。
- `src/bin/cli_handlers/libs.rs` 切换为共享渲染入口，收敛本地/远程 header 打印样板。
- `src/bin/cli_handlers/mod.rs` 注册新模块。
- `src/cli_args.rs` 对齐 `libs readme` 参数与 `docs` 系列的一致性。
