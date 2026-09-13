# 用 Markdown 明确 CLI 人类输出边界

- 含代码的 human 输出直接收敛为 Markdown-compatible 文档，不增加 `--markdown` 或新的顶层命令；版本化 JSON envelope 继续作为稳定机器契约。
- 共享 fenced-block renderer 会根据内容中的反引号长度选择安全 fence，并为 Cirru、JSON AST 与普通文本声明语言。
- `query def/peek/context/type/type-at/examples/tests/schema/search/search-expr` 与 `tree show` 的源码、schema、表达式和 chunk fragment 使用明确 code fence；元数据、截断状态和下一步命令留在 fence 外。
- `query def --json` 保留“human 后附 JSON”的兼容行为，但 JSON 改为 fenced block；自动化继续使用 `--format json`，stdout 保持单个 JSON value。
- Rust integration test 用于验证 CLI stdout/stderr 与 Markdown/JSON 序列化协议；这不是可由 Calcit definition `:tests` 表达的语言语义。
