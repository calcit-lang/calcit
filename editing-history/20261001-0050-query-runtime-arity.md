# 查询中保留 Proc 调用数量证据

关联 #1568、#1458。冻结清单不能仅把 Fn schema 的参数槽数量当作必需参数个数：`read-dir` 的 Bool 可省略，`trim/range` 等也已有独立 Proc arity 元数据。值类型与传参省略不是同一语义，不能改成 nullable/Dynamic 来表示。

`query def/context` 对直接注册的 core Proc 值和既有 runtime-implementation metadata 输出 runtime-arity；复用 CalcitProc::arity，包含 min/max，不新建 arity registry，不追踪普通 wrapper 或任意跨模块 alias。不能证明时不提供字段，不凭名字或仅靠 schema 推断。运行与类型检查不改。

CLI human 保持 Markdown-compatible，EDN 使用 kebab-case key，JSON 是既有 Agent 接口测试的显式兼容路径。Rust 覆盖 source 到注册元数据与序列化边界，Agent 测试验证两种现有查询的 envelope；已有 core Calcit :tests 继续验证一/二参数读取和 trim/range 的实际行为。

这只是冻结契约导出的前置证据补齐；完整公开 API 清单、别名移除排期与 CI 防止契约漂移的门禁仍须完成，不把此字段宣称为所有函数的完整签名导出。
