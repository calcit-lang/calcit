# 暂存 schema 保留 struct 的名义身份

关联 #1970。Reel 的合法 `reel.typed/Record<Op>` 能通过严格检查，但 `fix` 预览在类型解析后写入临时 Snapshot，将已解析 struct 的限定名缩短为 `Record`，触发旧内建类型名拒绝。独立 Snapshot 已复现；原程序及附带断言均通过，失败只发生在预览暂存序列化。

## 决策

复用声明已有的 `definition_ref`：嵌套 schema 的 struct EDN 序列化保留限定名和泛型参数，已解析的无参数 struct 值同样保留身份；应用类型的 Calcit 表示按既有 enum 路径使用带引号的限定名。没有 source 身份的局部声明保留原拼写，不推测命名空间。

不改变类型证明、Snapshot 顶层宽类型标记或旧 Record/Tuple 的拒绝策略，不修改下游公共 API，不新增 fix、checker、诊断或类型豁免。该修复是 API 合同收敛的消费者前置问题，不代表 #1953 完整家族冻结已完成。

已有历史合同中，core 方法的嵌套 `FsPath` 调用类型原来保存为短名；修复后保留 `calcit.core/FsPath`。沿现有合同表示归一化路径，只用历史基线已冻结的 core struct/enum 声明证明这两种拼写的同一身份，而且仅应用于来源为 core 的方法调用类型。源 schema、声明、其他命名空间和词法泛型仍严格比较；不重写基线，不从新添加的声明推测旧合同，不允许 Dynamic、字段或参数关系变化。

## 验证

在现有 serializer 单测覆盖不同命名空间的同名 struct、泛型参数、无参数表示、嵌套 List/函数和无来源局部声明。现有 fix CLI 集成测试通过结构化命令生成最小程序，用户语义保存在 definition `:tests`，验证只读 EDN/preset 预览、revision 拒绝、受保护应用、重新加载、原严格检查、原附带断言及幂等；保留旧类型名和错误命名空间的严格负例。

完整 Rust 和开启改写后校验的 check-all 仍为 PR CI 门禁，本机有限磁盘下的定向 harness 不冒充完整 cargo test。
