# 泛型方法查询保留实现证据

原 Option.map 方法调用能推导 Number→String，但 query 优先使用 Mappable 的开放 callback 声明，显示了原接收者返回类型。Agent 因此无法从查询获得已有的实现泛型关系。这是查询证据丢失，不应增加 Option 专用类型检查或放宽 Dynamic。

复用原方法派发与 schema。只有实现具有更少开放类型、符合原 trait 兼容关系，且 trait 的接收者及 where 约束可证明时，才用实现细化开放声明；trait 与实现的泛型处于独立词法作用域，不按同名变量串接。trait features 合并保留，闭合 trait 合同仍优先。实例化接收者后，已绑定 where 约束还必须满足通用严格类型证明；未绑定的约束继续暴露在查询中。

已特化的共享调用合同从已有类型节点收集自由变量，用原 generics 字段展示，不改写 Result.map 的既有调用节点或历史基线。Option.map 作为新增条目加入现有 query 导出的范围与基线；整个 API 家族冻结仍须后续逐族验收，本次不宣称完成 #1953，也不提供历史合同豁免。

语义放在 option:map 的 Calcit :tests，普通方法调用验证不同输出类型和未选分支不执行，在 native/JS/WASM 回放同一 AST。查询序列化和内部 schema 选择边界由原 Rust/Agent 协议测试覆盖；原严格负例继续拒绝错误 Option/Result 输出关系。Snapshot 通过 CLI 写入，随原 formatter 规范化的其他行只改变布局，不改变 syntax AST。
