# 2026-09-27 API 命名角色与 Agent 查询

## 决策依据

维护者要求逐步收拢类型、构造器、方法与内部函数的命名。现有查询把 `.unwrap`、`.includes?` 等方法列出时也展示 `option:*`、`&str:*` 的实现路径；Agent 容易把路径当成平等的公开调用形式。`%some` 等构造 helper 的元数据此前没有角色标签，`Option`/`Result` 核心定义反而带 `:internal`，单看标签不足以理解公开类型与内部实现的区别。

## 本次边界

先解释现有 API，不改调用语义、不增加顶层命令，也不批量重命名。四个 Option/Result 构造 helper 增加 `:constructor` 标签；类型与内部实现文档说明推荐调用。`query type` 的 human 输出指出 `.method` 是调用入口，实现路径只用于追踪；结构化 envelope 保持原有字段，角色通过已有 tags 表达。String 的 `.contains?` 检查字符索引，`.includes?` 检查子串，作为后续迁移试点候选，具体新名字仍须验证。

不删除 `option:*` 等明确开放边界可能需要的底层函数，不把所有 `&` 名称机械映射到公开方法，也不以 Rust 拼写替换 Calcit 的 kebab-case 与谓词标记。查询返回 `open` 或 `ambiguous` 时不能宣称方法的类型契约已证明。

## 验证方式

- 新指南的 Cirru 代码块用 `calcit docs check-md --snapshot src/cirru/calcit-core.cirru` 执行。
- core 的 definition-attached `:unit` 测试验证 metadata 改动没有改变执行语义。
- Agent 接口测试验证 constructor/internal 标签、`query type` human 提示及既有 EDN/JSON envelope。
- Quamolit 的 `quamolit.gpu-scalar-program/first-slot` 和 Timegrass 的 `app.server/main!` 用已安装的 `calcit query context --format edn` 查询真实 Option/Result 返回值与方法调用；不修改消费者仓库。
- 按 AGENTS.md 完成格式、clippy、Rust 和 `yarn check-all` 回归，PR 合并后核对 main 精确 SHA 的 Actions。
