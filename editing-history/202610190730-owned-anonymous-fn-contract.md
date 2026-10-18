# 匿名定义的输入上下文与实际返回证明

## 问题与决策

以匿名 `fn` 为代码根节点的 CodeEntry 已有自己的函数 schema，普通预处理使用这些参数信息，source-proof replay 却丢失上下文。因此合法的 Number 运算或 Countable 方法在完整 strict 中被当成 Unknown；另一方面，匿名函数的实际返回没有经过顶层值合同验证，`nil` 可以冒充 `Option<Nil>`，String 可以冒充 Number。

共享一个定义级词法 scope，同时用于普通预处理和拥有真实 source 的 definition replay。只从当前定义读取输入合同，进入时清除调用方的期望类型与宿主权限，结束或 unwind 时恢复；不让调用方合同成为函数实现的证明。

复用既有顶层值的方向性 proof，将匿名根纳入实际返回比较；继续使用现有诊断和源位置，不新增 proof 账本、计数或入口。开放证据的普通检查策略本次不切换，普通检查与完整 strict 的剩余差异仍需后续收敛。

## 异步边界

实际函数的调用属性来自其直接函数体。复用已有有效定义 schema 解析，修正原始 `fn` 函数体从索引 2 开始、具名/展开后函数从索引 3 开始的区分，保留首条 async hint。逻辑返回类型不变，JS Promise 与同步回调不能混同。嵌套匿名函数的 async hint 不属于外层函数。

## 验证方案

- 失败 Snapshot 只用官方 CLI 的 query、dry-run、scoped revision 事务生成；既有 strict-default 入口验证 nil/Option 和 String/Number 矛盾在原定义位置拒绝。
- 用户语义放在 fixture 定义的 `:tests`，用同一运行器在 native/JS 重放 Number、None/Some(nil) 的值与身份、Countable 泛型方法，以及独立嵌套函数。
- 在已有 strict-default 脚本验证真实 Promise 完成值和异步函数不能传入同步 watch callback，不删除旧异步数据测试。
- Rust 仅覆盖无法从合法 Calcit 程序观察的词法上下文 unwind 恢复，以及原始/展开后异步元数据位置。
- 完整 Rust、Clippy、集成回归与只读外部项目对照后再提交 PR；不把局部合同修复当成整个方向性证明统一完成，不回移维护版本。
