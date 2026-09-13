# 显式声明 syntax-node 输入格式

- 在现有 `edit`、`tree` 与 `cursor apply` mutation 上统一增加 `--input-format`，不增加新的顶层工具入口。
- 新自动化可显式选择 `cirru` 或 `json-ast`；旧的 `auto` 内容识别仅作为兼容路径保留。
- 保留 quoted Cirru 的 code/data 边界，并让 JSON AST 无歧义地区分 leaf、空 list 与普通 expression。
- mutation 写入前报告选中格式、节点类型、canonical JSON AST 和结构化摘要；格式错误报告预期与实际形状。
- 文档改为优先展示显式格式，端到端测试覆盖 `[]` leaf、空 syntax list、调用表达式与 quoted syntax。
