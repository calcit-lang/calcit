# 固定元数函数可用作变长参数函数类型（#2612）

memof 用函数作 memo 键并 `apply` 调用，因为 `Fn & Dynamic (-> Dynamic)` 不接受 `(fn (a b) …)`，只能写 `AnyFn`。Jon 2026-11-06 同意加这项能力，同时要求少用 `& Dynamic`。

- 规则：期望是 `Fn A… & T (-> R)`、实际是无 rest 的固定元数函数时，实际函数要能接收全部固定参数（最大元数 ≥ 期望的最小元数），多出来的每个参数都要接受 `T`（逆变），返回按协变检查。
- 改动集中在 `CalcitFnTypeAnnotation`：`accepts_call_range` 对“固定对变长”只比较最大元数；`compatible_signature_with_bindings`、`prove_signature_with_bindings` 不再因为实际函数没有 rest 而直接失败（多出的参数已在逐位置循环里对 rest 元素检查过）；泛型回调路径（`CallTypeProof`）对多出的参数逐个证明。
- 运行时实际传入几个 rest 值只有调用时才知道，元数不对仍然是运行时 arity 错误，文档建议用 `:tests` 覆盖。
- `& Dynamic` 已由 `W_OPEN_FAMILY` 计入 `Dynamic`，无需额外统计。

验证：新增 fixture `calcit/type-fail/variadic-fixed-arity-strict.cirru` 与 `strict_fixed_arity_callbacks_fill_variadic_slots`（接受 0/1/2 参数的数值回调；拒绝元素类型不符、固定参数不够、`FsPath` 参数接 `& Dynamic`）；更新单元测试 `fixed_arity_function_satisfies_variadic_contract_only_when_it_takes_the_fixed_values`。
