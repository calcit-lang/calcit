# 方法发现与 FFI 建议的准确性

#2441 来自真实 Cumulo Reel 迁移：正式 0.31.0 的 Enum `.assoc` 查询标为 proven，但严格调用拒绝旧无来源方法包。当前 main 的 `.assoc` 已因 Dynamic 参数显示 open，仍缺少真实分派限制；同一旧方法包的 `.empty?` 仍错误地显示 proven。查询的完整签名不应成为绕过真实分派规则的“可调用证明”。复用原先的 legacy/builtin 判定，在查询结果保留参数、返回和 definition 信息，同时将状态设为 open，说明既有错误与可选迁移方向。实际调用路径保持原检查，不复活旧 impl，不增加固定改写或新状态。

#2434 来自 calcit-http：共享 `:js-ffi` 标签也用于 native FFI。人类建议改称 FFI boundary；机器分类 `intentional-js-ffi`、schema、计数与权限保持不变。

验收包含真实 Enum 查询和严格拒绝、同一操作的公开 assoc 成功，以及原 List/Map/Struct 查询回归。已对 native HTTP 与 WS-EDN 的实际元数据逐项比较，除建议中的 JS FFI → FFI 外保持不变；模块缺失依赖的查询结果不冒充完整 strict 或业务验收。CLI 元数据回归放在现有查询测试，复用原 agent-interface 与完整检查，不增加 runner 或统计门禁。
