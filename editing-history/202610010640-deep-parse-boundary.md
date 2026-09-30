# 开放解析与深层类型验证的验收

关联 #1456、#1458，作为 0.28 的契约回归；不提前启用 #1538 的 0.29 默认收紧。

解析合法 Cirru EDN 可以返回 `Result<Dynamic,String>`，但语法成功不是 payload 的业务类型证明。
复用已有 `try-parse-cirru-edn-as` 与 `try-decode-map-as` 的 DataShapeGraph 路径，
新增四个 definition `:tests`：直接 typed parse 的嵌套 List 成功/失败、开放解析后深层 decode 的成功/失败。
失败必须返回 err 并含 `[0][1]` 结构路径，不依赖整段错误文本，也不删除非法项或提供 fallback。

Node 脚本仅负责 backend 执行边界：从实际 Snapshot 查询相同测试 AST，通过现有 CLI
在临时 Snapshot 中装配入口，再运行严格检查、native 与真实 JS 产物。语义断言只写在 Calcit，
不复制一套 JavaScript decoder 测试。脚本加入现有 `check-all`，没有新公开命令、统计门禁或依赖。

文档说明开放数据可以保存/传递，但具体使用前需要真实验证；目前方法查询的 `open` 不冒充 proven。
WASM 嵌套集合 parser 尚不支持，本批不扩大其能力。测试通过不等于通用 assert/call/Dynamic 漏检已全部修复。

验证记录在对应 PR；先运行官方 0.28.0-alpha.2 CLI 的新 attached tests，
再构建当前源码和 JS runtime，验证相同版本组合及仓库完整门禁。
