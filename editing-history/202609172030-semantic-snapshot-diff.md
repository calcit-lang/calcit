# Snapshot 语义差异证据

- 扩展已有 `calcit analyze program-diff`，不新增顶层 diff/query 工具。
- typed Snapshot 相同但原始内容不同的情况稳定分类为 `canonical-format-only`，避免把 formatter 布局变化误报为语义变化。
- entry/config、namespace/import、definition lifecycle、schema/signature、FFI boundary 与 executable expression 分别输出路径、definition 和 before/after revision。
- 人类输出继续使用 Markdown/tree；自动化可选择 Cirru EDN，JSON 仅作为显式互操作格式。
- 报告保持只读，不猜测缺少可靠 identity 证据的 rename，也不替代 strict check、测试或行为验证。
