# 静态特化参数保留真实函数目标

## 故障与选择

WASI 的原 `&list:sort-by` 函数键测试仍被间接调用门禁拒绝，而相同源码已在 native、JS、WASM 运行。既有静态特化为参数分配了新 local，也保留参数类型，但没有像 IIFE/不可变 let 路径一样保存实际函数定义目标。捕获参数的比较器因此退回函数表间接调用。

在原参数绑定处复用现有 `resolve_known_function_value` 和按 local 槽编号保存的目标表。只记录实际表达式可以证明的函数定义，不从签名、名称或返回类型推断函数目标或 effect。原参数仍按顺序求值一次，没有重排表达式或重写语言源码。

只对无 `recur` 的特化参数记录该证据：这类 fresh local 在调用体内不可变；循环参数可能改绑，不能把首次目标当作永久不变量。命名递归、闭包、未知间接目标和宿主能力仍经过原有边界，不放宽 dependency 校验，不新增 ABI、公开 API、诊断编号、Dynamic 或 unsafe。

## 验证与剩余范围

- 原 core 函数键测试通过真实 WASI 执行后，仅删除它的精确 WASI 排除。其他三项同诊断用例仍被拒绝，保留原排除，不宣称整个间接调用任务完成。
- 在现有函数值 fixture 的 definition `:tests` 验证具名函数、不可变别名、相同签名不同目标的遮蔽、前后独立调用、空输入和接收者求值一次。复用既有 `known-function-alias` tag 的四后端 CI，不增加 job 或测试运行器。
- 扩展已有 namespace/callee 低层测试，检查真实目标可降为直接调用、相同签名但无目标证据仍是间接调用，以及真实直接目标仍被宿主 capability 门禁检查；不新增 Rust 测试数量。普通排序语义保留在 Calcit 侧，不改成 native call。
- 旧 main 固定二进制回放同一新增源码，native/JS/WASM 通过而 WASI 失败；原型四后端通过。Snapshot 使用官方 query、guarded transaction、完整 readback 和 program-diff，没有签名放宽或删除旧测试。

`map` 的 Set 路径、返回闭包的组合、捕获 MapEntry 的排序等仍各有边界。本次保留原问题与其真实失败，不用新增排除或宽泛的 effect 假设替代后续修复。
