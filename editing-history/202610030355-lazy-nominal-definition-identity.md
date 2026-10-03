# 首次求值与缓存共用名义定义身份

## 原因

`write_runtime_ready` 会为没有来源的 StructDef、EnumDef、Trait 添加声明路径，
但 thunk 首次求值返回的是写入前的原值。`impl-traits Base Impl` 首次取得
没有来源的 Base，别名写入时便被赋予别名自己的路径；后续缓存取得的 Base
已经有原声明路径，导致相同数据声明在类型检查中不再相同。

## 决策与边界

将既有身份赋值提取为一个内部函数，thunk 在保存和返回前先调用它。
已有身份保持不变，trait 附加不产生新的数据类型；字段、泛型约束与方法实现
不变。缓存写入仍使用同一规则。没有改成结构相似即可兼容，没有放宽
Dynamic、插入 cast 或额外执行尚未求值的 producer。

早期在类型解析层补来源的尝试没有通过复现，已撤回；修复位于证据首次丢失
的求值边界，而不是让消费端绕过身份检查。关联 #1709、#1553。

## 验证方式

扩展既有 typed-source-alias fixture 的 definition-attached tests：带 trait
的 Struct/Enum producer 可传给基础类型 consumer；原构造器继续可用。
同一组 Calcit 表达式由现有 runner 在 native 与生成 JS 中执行，另保留
不同声明、跨命名空间同名结构体、错误参数、惰性 producer 与循环别名检查。
CI 复用该 runner，不增加脚本或测试框架。

正式 0.28 的两个新正例均失败，补丁的八项附加测试及 native/JS 回放通过。
ws-edn 真实前端 main/reload 检查、完整前端 JS 生成和原四项 unit 通过；
没有启动浏览器、WebSocket 服务、绑定端口或操作用户存储。

fmt、clippy 和 Rust library 检查通过。CLI 协议检查在 strict workflow
manifest 失败，正式 0.28 同一检查也失败；不得称全量验证通过。
本地端口测试不执行，其他完整结果与 CI 结果记录在 PR。
