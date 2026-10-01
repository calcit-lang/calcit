# reader 展开后的 receiver 类型与迁移证据

关联 #1588、#1458。真实 Respo 的 Number 计数器 `.add @*resource-id 1` 被两个集合
migration rules 列为 review-only，query type-at 对 receiver 返回 unknown。没有错误写入，
但 Agent 必须人工排除编译器本来可证明的 Number，违背减少迁移噪音的目标。

根因有两个：reader 将 `@value` leaf 展开成 deref call，head/operand 共用原 leaf 的
source coordinate；普通 call 的 head 去掉一层索引才是表达式坐标，对 reader 不适用。
此外原始 leaf 对 compiled tree 的形状查找可能命中 Atom operand，而不是其 payload。

修复既有 call-location 推导中的 reader 情况，使 source-expression trace 保留真实 leaf
位置。query 对 reader 展开的 call 使用唯一 trace evidence；集合 fix 跳过这一情形的
raw shape lookup，复用既有 unique trace。没有从外围返回值猜 receiver、没有新 API
registry/规则入口、没有按项目名隐藏结果。Number.add 原样保留；List/Set 的合法迁移
仍须通过原 implementation/source-context/revision/fingerprint 门禁。

方法 call 的 head 本身不携带 source location。已有 trace 现在也记录能够从 receiver
恢复的 method call；query 优先唯一 trace，避免把 reader 内层 deref 误归为外层 add 的
lowering。找不到唯一 source evidence 时明确 unavailable，不从相似位置猜 callee。

CLI regression 通过现有 mutation commands 建临时 Snapshot，挂三个 Calcit :tests 验证
Number/List/Set atom 的 deref 与 add 语义。Rust 只核对 query envelope、具体 payload 类型、
预览不写入、Number 无集合候选、List/Set 有可证明建议及 guarded apply/repeat-empty。
正常与宏源码无法唯一对应时继续保守；此修复不代表消除所有开放类型或宏迁移建议。
