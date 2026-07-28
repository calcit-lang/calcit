# Agent 工具链 RFC 拆分

## 概要

- 移除体量过大的 `07-26-agent-semantic-interface-roadmap-rfc.md`，按紧密关联的功能边界拆分为文档与评估、机器协议、安全结构化编辑、静态语义分析等 RFC。
- 补充单项目工具契约、Git 模块存储与持久树形 cursor RFC，明确 Calcit 继续以 EDN/Cirru 树形源码、结构化文档和既有 `cr` 子命令为中心。
- Git 模块仍以 tag 为最佳实践、允许分支名，不引入 registry、workspace 或 lockfile；依赖冲突采用最高版本并输出 warning。
- LSP 延后到一次性解析接口足够稳定且维护成本可接受之后，不把行号作为源码或诊断的核心身份。

## 文档边界

- 新 RFC 各自描述一块可独立评审和实施的能力，避免实现状态、长期设想和不适用于 Calcit 的 Cargo 风格命令混在同一文件中。
- `RFCs/README.md` 更新索引，后续实现和验收可直接引用对应 RFC。

