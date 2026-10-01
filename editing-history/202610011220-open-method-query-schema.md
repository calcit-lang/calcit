# 保留开放方法的已声明类型边界

关联 #1456 / #1458，0.28.0 收尾。String 的解析方法声明了确定的 Result 外层和 String 错误类型，但成功值允许开放数据；原查询因包含 Dynamic 而同时隐藏全部参数与返回类型，容易让 Agent 误判整个 API 没有 schema。

复用现有方法分派、接收者绑定与 schema 替换结果，在成功解析的开放契约中保留已有类型字段。不新增 registry、命令、证明状态或类型规则。缺失 schema、DynFn 整体不可调用、接收者绑定失败及歧义仍不给签名。开放 payload / callback 不改为具体类型。

`status = open` 和现有原因说明保持不变；结构化 `call-types`、自动 fix 及核心 API 冻结只接受 `proven`。字段存在表示可发现的声明，不代替闭合调用证明。已冻结的 proven 契约与候选范围不变。

这是 CLI/内部元数据边界调整，Rust 验证查询展示、绑定失败与缺失证据，现有 Node Agent 协议检查验证 EDN/JSON 等价，复用已有 Calcit 解析 `:tests` 验证 native/JS 语言语义，不重复添加 Rust decoder 行为测试。
