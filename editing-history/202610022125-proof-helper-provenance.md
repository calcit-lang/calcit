# Proof 错误的 helper 来源证据

## 背景

现有诊断已经拥有 `provenance`，query、fix、strict workflow 也会序列化它，
但编译器未填充实际来源。最小复现中，Dynamic 参数经过 source、helper 和 let binding，
assert-type Number 正确拒绝，报告却只有 consumer 的位置与空来源链。

## 方案

在已有显式 proof 审计中保存词法绑定的解释，错误发生时附到原诊断。
只读取实际预处理表达式、解析到的 Import、已编译 helper body 与现有类型推导；
不执行程序，不另建符号 resolver，不新增诊断编号、迁移规则或 CLI 入口。
断言输入、具体调用实参与函数返回共享这一解释层。

来源只是导航信息，不参与 TypeProof，不能用 producer 的声明或来源条数证明类型。
同名变量用 namespace、definition 和词法 scope 恢复处理；成功、错误和 unwind 均恢复旧绑定。
对 helper 循环保留访问集合和深度限制；嵌套 let 不重新启动递归预算。
最多展开八层并保留十六项；条件实参只是可能贡献，不表示精确运行时分支。
没有真实 source coordinate 的生成节点不猜路径。

## 验证

协议与 source navigation 检查扩展已有 Rust CLI 测试，因为验证的是诊断序列化、
EDN/JSON、作用域隔离与未写回，不新增重复用户语义测试。
既有 Calcit definition `:tests` 与跨后端 runner 继续验证语言语义及拒绝行为。
测试覆盖直接开放参数、泛型、callable、循环断言、跨 helper、局部遮蔽、
scope 恢复、递归 helper、preview/apply 不写回。完整结果写在 PR，不将计划写成通过。
