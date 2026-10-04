# 继承方法的查询与静态分派一致

## 问题与决策

js-ffi 的 KeyboardEventHost / MouseEventHost 已通过 `('requires EventHost)` 继承事件方法。匹配已发布的 0.29.0-alpha.1 时，正常 Calcit 调用及 Respo 跨模块消费者能通过检查、Node 和 Chromium 运行，但 `query type KeyboardEventHost` 的 methods 为空。Agent 因而无法从类型发现可调用的方法。

抽出正常分派已有的、经过签名校验且按名义来源去重的 trait closure，让方法列举与方法选择共享它。保留原分派优先级及 open / ambiguous 判断，不增加语言规则、入口、元数据注册表或诊断编号。查询只补方法，字段不伪装成方法；宿主名与字段查询不是本次范围。

## 兼容边界

普通 trait、external-object trait、传递 requires 和 TraitSet 都遵守相同规则。重复继承路径按 origin 去重，不按短名称合并。继承 DynFn 仍为 open，同名不同来源仍为 ambiguous；cycle / 不完整签名不能被列举为可用的完整契约。此变更不验证任意 JavaScript 实现，也不修改受检转换、宿主种类 decoder、Dynamic 边界或 backend 支持。

## 验证方式

已有 Calcit `test-traits` 与 `js-ffi-module` 的 definition `:tests` 保留语言和宿主语义；本次不新增语言行为。扩展现有 Agent 接口运行器，查询真实 Greeting / TallyHost 定义并比较 EDN / JSON；Rust 回归只检查内部 closure、diamond 去重、字段排除、open / ambiguous 和非法元数据边界。真实 js-ffi / Respo 消费者副本用于补充查询与可执行方法的一致性，不把全量 Respo alpha.1 的已知失败写成通过。
