# 结构修改输出统一为 Markdown 边界

对应 issue #1077。

## 变更

- 复用共享 renderer，为 `tree` mutation、`cursor apply` 与 `fix` preview 输出明确的 Markdown heading、metadata 和 fenced Cirru 源码。
- `--expect` guard 失败时，在 stderr 中分别输出 Expected/Actual Cirru fence，并明确标记 `changed: false`。
- `edit` 的显式语法输入摘要同时展示 canonical Cirru；只有调用方明确输入 JSON AST 时才附加 JSON fence。
- `fix` 与 `edit transaction` 增加 Cirru EDN 结构化输出，复用与 JSON 相同的语义 report；JSON 保留为显式互操作格式。
- 文档默认示例改用 EDN，并补充 Agent 的 read → preview → apply → verify 流程。

## 验证

- `cargo test --test human_markdown_cli --test edit_cli --test fix_cli`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `yarn check-agent-interface`
- `yarn check-all`
