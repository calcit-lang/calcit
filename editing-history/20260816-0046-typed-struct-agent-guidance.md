# 2026-08-16 Agent 的 Struct 字段访问与临时文件引导

- 已知具名 Struct 的应用代码应使用 `(:field value)` 或 receiver-first `value.:field` invoke 语法；检查器会保留字段声明类型，并继续降为索引访问。
- 直接使用 `&struct:get` 现在会报告 `W_STRUCT_RAW_ACCESS`；接收者无法静态解析时报告更严格的 `W_STRUCT_DYNAMIC_RAW_ACCESS`。可复用 `defimpl` 与 core/runtime 动态边界保持兼容。
- 本地 `let` 中的 `defstruct` / `defenum` 现在可解析函数参数、`assert-type`、泛型 enum 构造及 `match` payload 的名义类型，避免类型退化为 `Dynamic` 后迫使代码绕过检查。
- CLI 收到 `/tmp/...` 或 `/private/tmp/...` 的 Snapshot/`--file` 路径时在 stderr 提示使用项目内 `.calcit/snippets/`，并提醒将 `.calcit/` 保持在 `.gitignore`；查询命令的 stdout 协议不受影响。
- Agent、Struct、Enum、Trait 与 quick reference 文档和相关 Snapshot 回归均改为优先展示类型化字段访问。
