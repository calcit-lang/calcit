# RFC: 结构化文档上下文与 Agent 工具链评估

状态：Draft
日期：2026-07-26
关联：`07-19-doc-knowledge-index-rfc.md`、`07-26-agent-machine-protocol-rfc.md`

## 1. 文档仍是结构化事实

Markdown frontmatter、heading 与 snapshot definition metadata 都是可解析的树形数据。文档系统应保持 Markdown 可读、Git 可管理，同时从它们派生可删除重建的索引；不要求文档退化成纯文本检索，也不把 cache 当事实来源。

为每个 definition 自动派生：

```text
calcit://definition/<namespace>/<name>
```

节点包含 doc/schema/examples/tags。Markdown 的 `id`、`parent`、`related`、`requires`、`leads_to`、`code_refs` 负责概念入口与精选关系，不要求作者手工列举全部 API。

每个 heading 派生 section node，使用稳定 slug、内容 hash、层级、正文范围、代码块类型和 definition refs。这里的“范围”仅用于读取当前 Markdown section；源码定位仍遵循 definition/tree selector，而不是行号。

## 2. 统一 frontmatter 与默认检索范围

前端搜索、知识图和校验必须复用同一个版本化 frontmatter parser。逐步校验必填/enum、duplicate ID、dangling edge、scope、可解析 code refs 与可执行的 current 示例；旧文档可兼容读取，再分阶段变严格。

默认 `cr docs search` 只覆盖 current guide/reference 与 definition metadata。RFC、草稿与 `editing-history/` 必须显式带 scope 才进入结果，避免历史语法污染 Agent 上下文。

## 3. 可重复的 Agent 基准

`yarn check-agent-interface` 作为真实 CLI 进程测试，至少记录：成功率、工具调用数、stdout bytes/token、无效命令、重定位/语法重试、修改后回归数和耗时。

任务集覆盖：静态 type/method 查询；definition+schema+examples+docs 导航；重复 leaf 的指定 tree 编辑；新增 definition/import；schema 或 method 诊断；JS FFI intentional dynamic；changed verification；revision 冲突拒绝；从诊断到最小 example。

每项 Agent API 改动必须同一任务集前后比较。是否引入 daemon/LSP、是否调整 context budget，均以这些测量数据决定。
