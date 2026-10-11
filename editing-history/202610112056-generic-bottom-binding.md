# 泛型调用中的空 payload 推导

calcit-regex 的原有 `Fn<T>(T,T)->Unit` equality helper 在先收到 `Option :none` 时，把 T 固定成 `Option<Never>`，随后拒绝 `Option<String>`。正式 0.29.0 与 #1982 候选均复现该独立问题，关联 #1981。

使用已有类型关系判断先前含 Never 的闭合绑定是否是后续参数的可证明子类型；只有证明成立才能细化绑定。保留真实 payload 不匹配、Ref 不变性、名义身份及调用者泛型边界，不增加迁移规则、诊断编号或业务类型例外。

正常行为写入现有 same-type-second 定义的四项附带测试，涵盖空 Option 在前/后、空 List 和局部泛型调用。在正式 0.29.0 上为 2/4；候选在 native、JS、WASM 均为 4/4。失败行为扩展既有 strict-default runner，拒绝 Option payload 不匹配与 Ref 内空 payload 的不变性冲突；既有底层 Rust invariant 测试增加 CallTypeProof 边界断言。

统一回放运行器原先用固定缩进字符串定位 Snapshot 命名空间，无法读取官方 CLI 写出的单命名空间 canonical 布局。复用已有 Cirru parser/writer，通过结构节点插入临时测试，不增加依赖或独立脚本；不为运行器改写业务源码。现有 core 回放通过 native 613/613、JS 612/612、WASM 388/388，原排除项不变。

现有 strict-default 完整检查已通过；Rust、Clippy、Agent interface、完整 check-all、最新 head review 与精确 main CI 仍按仓库门禁继续验收。下游完整原生入口必须使用同时包含 #1982 与本修复的候选验证；未发布候选不作为正式依赖版本，也不宣称 regex 发版完成。
