# CLI 结构化输出优先使用 Cirru EDN

本次为 milestone 0.14.18 的 #1080 明确 CLI 输出层次：不传格式参数时仍为适合人类 review 的 Markdown-compatible human 输出；显式请求结构化 Calcit 数据时以 Cirru EDN 为原生首选；JSON 继续作为显式互操作格式。

## 实现

- 抽出小型共享 `StructuredOutputFormat` 解析层，先由 `analyze verify` 与已有 `edit scaffold` 共同使用，避免继续复制字符串分支或增加顶层入口。
- `analyze verify --format edn` 复用现有 JSON envelope 的同一份语义值，再映射为带 tag 与 kebab-case key 的 Cirru EDN；现有 JSON 字段和行为不变。
- 成功与配置失败路径都保证 EDN stdout 是单一可解析文档，并与 JSON 对照核心字段。
- 更新 Agent 与验证文档，把 EDN 放在 Calcit 自有自动化示例的首位，同时说明旧命令迁移期间以及 JSON-only consumer 仍可显式使用 JSON。

## 后续边界

config/query/fix/cursor 已有结构化输出将逐步复用同一契约；本次不一次性改动全部命令，也不增加格式统计、隐式协商或另一套 analyzer。
