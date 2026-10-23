# 函数内 defstruct 以 let 绑定作为名义身份

#2121 记录了一个独立证明的假拒绝：函数内用 `let` 绑定的 `defstruct` 没有 source `definition_ref`，构造 `Payload :value 7` 时预处理把绑定上的定义复制成新对象，于是 `hint-fn` 参数 `'Payload` 与实参虽然来自同一个绑定，也只能报告 `expected struct LocalPayload, got struct LocalPayload`（`E_CALL_ARGUMENT_UNPROVEN`）。native 与 JS 都能正常执行。

构造路径现在直接共享绑定上的定义对象，`struct_nominal_match` 原有的同一对象判断即可证明。没有 source 身份的两个定义仍返回“无法判定”，所以同名同字段的另一个局部 `defstruct` 继续被独立证明和完整 strict 拒绝；不按名字或结构相同补发名义证明，也不放宽 Dynamic。WASM/WASI 仍不支持运行时创建的一等 StructDef。

验证：`calcit/test-enum.cirru` 的 `read-runtime-local-type` 在 native/JS 回放（`:local-type-shadow`），`scripts/check-known-assertion.mjs` 对它运行 `callable-contract-proof-v1`，并对两个同形局部定义的反例断言独立证明与 strict workflow 拒绝。
