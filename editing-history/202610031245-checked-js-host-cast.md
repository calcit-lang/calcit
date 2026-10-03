# 外部 trait 的受检宿主转换

关联 #1704、#1694 与 js-ffi #154。沿用已定案的 `js-cast value 'Trait`，不新增外部类型种类、宿主种类表或 defexternal 词汇。

转换处只检查由已有 field/method 声明推导的成员形状，复用 :names 与默认 host 名映射、名义身份及 requires 图。成功保留对象身份，表达式只求值一次，不写属性、不调用方法。外部签名仍是 FFI 声明的信任边界；成员存在不能证明返回类型或 Promise payload，不把减少 unsafe 数量作为类型证明指标。

词法 :js-ffi 是硬约束。普通 trait 与数据类型不作为转换目标；native/WASM 明确不支持，不静默擦除 cast。字段 presence 使用 in，不触发 getter；方法需要读取属性以验证 callable，getter/Proxy 异常以统一 TypeError 保留 cause。

用户可见字段/方法语义放在现有 JS FFI Snapshot 的 definition :tests，既有 runner 查询实际 AST 并回放生成 JS。仅宿主 nullish、冻结对象、原型与 getter/Proxy 等低层 invariant 在同一 JS runner 检查，不新增 check 脚本。完整 CI、静态错误与真实消费者回归尚待完成；本记录不宣称 #1704 所有子任务或 Promise adapter 已解决。
