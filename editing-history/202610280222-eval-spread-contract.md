# eval 与 Snapshot 的字面量展开参数检查

## 原因与决策

#2432 的带注解 `List<Number>` 回调在 `eval` 中能用 `.concat` 的字面量展开追加 String，随后才在数字运算中失败。普通 proc 参数检查遇到 `&` 直接返回；Snapshot 默认额外证明会拒绝，而 eval 的普通检查留下缺口。

普通 proc 检查复用已有 `expand_literal_call_arguments`，对静态可展开的实参运行原有数量、宿主身份和类型检查。只转换检查视图，不改变运行 AST、求值次数或错误传播。不为 eval 新增审计模式、诊断、例外或固定改写规则。无法静态展开的调用保持已有证明与运行时路径。

## 验证与兼容边界

定义 `:tests` 覆盖同型追加、空/多个展开以及显式 Dynamic 列表传递；现有跨后端运行器回放 native、JS、WASM，WASI 沿用原 core 测试工作流。现有 spread 脚本验证 prefix/postfix/proc 写法、位置参数/stdin eval，以及 Snapshot 检查和执行/codegen：在额外证明开关两种状态下都必须于副作用之前报告已有类型诊断和源码位置。

新增静态拒绝属于 0.32.0，不回移为 patch。既有异构构造和显式开放存储合同保持不变；用户应修正不匹配的实参，不能用展开绕过已有函数合同。
