# 2026-07-19 22:36 文档知识图谱与增量缓存

## 修改概要

- 为 `cr docs` 增加文档知识图构建、检查、关系遍历、路径查询、定义反查、缺失定义和孤立节点命令。
- 使用 Markdown frontmatter 的 `id`、`parent`、`related`、`requires`、`leads_to`、`code_refs` 表达可 Git 管理的知识关系。
- 在 `~/.config/calcit/docs-cache/` 生成 JSON 文本缓存，按文件内容、解析器/schema 版本和内置 Calcit snapshot 指纹失效。
- 为文档索引 RFC、`CalcitAgent.md`、索引规范和验证案例补充使用说明。
- 为核心数据结构文档补充稳定节点 ID 与 Calcit 定义引用，验证从概念到 API 和编辑工作流的跳转。

## 验证

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -q`
- `yarn compile`
- `cr docs graph build`
- `cr docs graph check`
- `cr docs graph path core/features/list core/run/edit-tree`
- `cr docs graph explain calcit.core/nth`
- `cr docs graph missing`
- `cr docs graph orphans`

## 后续方向

- 将 `missing` 从总量报告扩展为按公共 API、语法符号和内部定义分类。
- 接入模块或应用级 Calcit snapshot，并增加示例覆盖检查与 LLM context 聚合。
