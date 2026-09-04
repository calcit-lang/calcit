# Remove redundant specialization clone / 移除专门化中的冗余 clone

Copilot's third review found that the dispatcher cloned all expected argument types before branching, even though `update` delegates to a helper that performs its own necessary clone. The clone now lives only inside the `get` and `includes?` branches that mutate it.

The same review summary identified inconsistent test-only `CalcitFn` metadata. The helper and the two existing update fixtures now keep `args`, `call_shape`, and `arg_types` at the same arity so future tests cannot accidentally rely on contradictory callable metadata.

Copilot 第三轮 review 指出 dispatcher 在分支前复制全部 expected argument types，但 `update` 会委托给自行执行必要 clone 的 helper。现仅在真正修改副本的 `get` 与 `includes?` 分支中 clone。

同一 review summary 还指出测试专用 `CalcitFn` 元数据不一致。helper 与两处既有 update fixture 现保证 `args`、`call_shape`、`arg_types` 的 arity 一致，避免后续测试依赖互相矛盾的 callable metadata。
