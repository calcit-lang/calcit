# 无 payload Enum 的局部推导

关联 #1737、#1553，延续 #1736 的严格字段合同和跨 namespace review 回归。

## 真实反例

`Option :none` 直接用于具体字段合法，但将同一表达式绑定为不可变局部变量再传入，被推导为 `Option<Dynamic>` 而拒绝。常量 `if true` 在预处理时选中空分支，同样丢失信息；不能把这个反例当成非恒定分支合并覆盖。另补真正 Bool 参数的分支测试。

## 决策

在类型推导中以内部 Never 表示未承载值的泛型槽，不用 Dynamic 表示已证明的缺席。只有已知变体、正确 payload arity、其 payload 未使用该泛型时才有此证据。参与变体却不能推断的槽仍保持开放，不能授予边界证明。沿用已有已编译 wrapper 的变体参与检查，不按函数名猜测，也不新增局部变量 registry。

Never 是无值证据，不产生运行时值，不参与泛型绑定；现有定向 proof 可将它用于具体期望，反向关系不能把实际值证明为 Never。Ref 的 invariant 双向 proof 继续阻断不安全转换。泛型 Enum 的名义身份仍使用已有声明身份，不以附加 impl 的完整对象相等替代声明身份。

可变 Ref 初始化不能把无值槽当作任意未来写入的授权。`atom (Option :none)` 推导的无值槽不再暗中成为 Dynamic；现有 `assert-type` 可以为初始化值提供具体上下文，但是否要求旧调用者添加此上下文尚未获确认。优先保留原写法并完善局部约束推导，不能通过强转已有 Ref 或放宽 invariant 关系替代推导。最终选定的非 patch 升级边界必须在指南和 PR 明示；测试同时验证具体 Ref 的合法写入与错误写入。

完整 core 回放中的 Result 分支测试还确认 mixed TypeRef / 已实例化 Enum 的兼容 arm 反向检查泛型参数。修正该共享关系的 actual/expected 方向，Struct 的同类 arm 一并对齐；不恢复 Never 与任意值对称兼容。字段检查不再自行补全泛型槽，分支合并也直接使用推导类型而不是重新检查 source 构造器。

生成 hint 和查询需要往返内部证据，使用内部 `:never` 表示，不增加公开 `'Never` source symbol 或新 CLI。它不是 Dynamic/Unknown 的别名，也不能用于 decode 一个实际值。不把本次修复扩成所有空集合、raise 和 todo 的完整控制流改造。

## 验证

现有断言脚本创建 definition-attached Calcit 测试，native 与真实生成 JS 回放局部变量、再次别名、常量分支、非恒定分支、跨 namespace wrapper 和任意用户 Enum 的空变体。负例保留开放 payload、错误 nominal 身份及原有递归边界，七种 native/check/JS/WASM/WASI 入口必须拒绝且无应用产物。内部泛型绑定与 Ref variance 用低层关系测试验证。

## 未完成的边界

完整 Cirru suite 的 `when-let` 通过 `option:fold` 使用多个 callback，前一个空 callback 的返回信息曾被过早当作后一 callback 的刚性期望。共同返回合同现在在普通和严格模式都生效；只有两个 callback 都有独立返回证据后才合并类型，不以部分返回证据声明另一个 callback。完整 native Cirru suite 和 definition-attached 严格 native / JS 正例通过，涵盖空值在两侧、任意 nominal 返回 Enum 与导入 wrapper；错误 fold payload 继续通过七种入口拒绝。未添加 Dynamic 或按 source 函数名授权的新例外。

候选尚不能合并：真实 Respo 回放新增 loop 初始空 Option 与后续 `recur` 的 Number payload 冲突（61/74，相比前候选 62/74）；应修复核心局部约束推导，不能删除下游测试或静默修改 loop 业务。最小复现是 `loop ((n 0) (value (Option :none))) (if (&< n 1) (recur 1 (Option :some 7)) (option:unwrap-or value 0))`，候选仍报告第二个 recur 参数的类型冲突。

Ref 的显式初值上下文已经可用，但是否要求旧写法补上下文、还是继续推导后续写入约束，已请求维护者选择。在确认方案和补齐上述回归前保持 Draft，不发布、不合并。
