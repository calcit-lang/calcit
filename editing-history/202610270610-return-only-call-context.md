# 泛型工厂的调用结果按期望合同实例化

Reel 的空状态工厂具有 `<Op,Store>(Store) -> State<Op,Store>` 合同。生产者的空列表字段已经可以证明，但调用推断会将未由输入绑定的 Op 转为 Dynamic，使直接断言、具体参数和声明返回均丢失关系。

复用 `expression_is_proven_for` 的期望类型检查和 `CallTypeProof`：每次调用使用已有 fresh callee binder，先保留输入约束，再按结果合同约束本次调用，重新验证所有输入及 where 合同，最后用刚性关系确认结果。不会绑定外围词法变量，不将真实开放输入伪装成具体值；没有期望类型的普通推断保持原行为。复用现有参数展开、arity、异步返回边界及 predicate evidence lifetime，不增加类型 AST、诊断、用户入口或迁移规则。

`let` 返回别名时，来自本次上下文实例化的临时证明只在现有作用分析确认局部证据仍有效时传播；重新赋值或未知作用不能复用修改前的证明。构造器原有字段证明保持不变。

正例扩展已有 `empty-field-box` 的 definition `:tests`，覆盖独立调用、声明返回、具体实参、外围泛型、透明 let、分支及函数别名，使用现有四后端运行器。现有回放默认不审计 fixture 的独立证明，因此仅此测试组显式启用 `CALCIT_PROOF_AUDIT=1`。无法执行的非法程序继续由现有 known-assertion 脚本验证编译拒绝、诊断归属、文件不变及无成功 JS 产物；不新增检查脚本或 Rust 语义测试。
