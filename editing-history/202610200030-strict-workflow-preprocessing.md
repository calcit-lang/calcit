# Strict workflow 验证使用当前类型语义

## 真实阻塞

Geometry Book 迁移后，普通 all-defs 检查、单条 proof 规则、原生测试与 JS 运行通过，完整 strict workflow 却在宏执行时报告对 Option:none 调用 `.filter`（#2199）。现有 CLI 测试中的最小类型化宏也能复现：宏调用接收 List<Doc> 的函数，函数在 mapcat 回调内读取 Option<List<String>> 字段并筛选解包后的列表。

## 决策与边界

workflow 原先无条件以兼容模式执行预处理，再运行严格 source-proof replay。兼容模式的类型推导影响宏的实际执行，产生普通严格语义中不存在的错误。

只读 `--verify` 仍逐定义预处理并收集诊断，但使用当前严格类型模式；preview/apply 保留兼容模式，继续为未迁移的旧代码收集规划证据。不能直接删除预处理，因为 source-proof replay 不能替代对重复 type-slot 声明的检查。保留原 proof、entry 与 verification profile 门禁，不增加 fix 规则、诊断编号或消费者绕过逻辑。

## 验证

现有 `tests/fix_cli.rs` 覆盖 CLI 编排、退出码与只读合同；使用带 scoped revision 的官方 edit transaction 安装最小用例。先证明普通检查和执行通过而 workflow 失败，再验证修复结果。重跑既有 strict workflow 正反例，特别是重复 type-slot、未证明断言、具体返回、回调合同与迁移规划。真实消费者回归和完整仓库门禁结果记录在 PR，不将 main 候选验收冒充正式 0.30.1 发布。
