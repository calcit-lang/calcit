# 证明审计使用生成 callable 的存储合同

默认预检查加入断言证明审计（见 `202610261900-directional-default-proof.md`）后，项目代码里对名义 Enum
调用公开 `contains?`（`contains? (Option :some 1) 0`、`contains? (Result :ok 1) 0`）在 `--check-only`
与直接运行时报 `E_CALL_ARGUMENT_UNPROVEN`：`calcit.internal/&option:contains?` 的第 1 个参数期望 `'T`。

原因：公开 `contains?` 对名义接收者内联到 `synthesize_nominal_impl_callable` 生成的 callable。它没有
源码条目，只在编译结果里保存 trait 方法合同 `Fn<T, K>(T, K) -> Bool`；重建的参数列表只带 `'T`、`'K`
类型，不带泛型声明。`effective_user_call_schema` 只查源码 schema，查不到时退回函数值本身，审计于是把
`'T` 当成未绑定的具体要求。`calcit eval`、`--all-defs` 与 core `:tests` 不经过这条入口审计，所以没有暴露。

修改：`effective_user_call_schema` 在源码 schema 缺失、且定义没有源码时，使用编译结果中存储的函数合同。
只影响编译器生成的 callable，源码定义仍以源码 schema 为准；生成合同本来就是这些 callable 的 type summary。

回归：`tests/fixtures/public-contains-nominal.cirru` 由 `scripts/check-strict-default.mjs` 做入口
`--check-only`、`--all-defs` 和带审计的运行，并用现有运行器在 native/JS/WASM 回放附带测试。

English: the proof audit now reads the stored contract of compiler-generated callables (such as the
nominal impl callable behind public `contains?`) instead of rebuilding a generic-free signature from
the function value, so project code calling `contains?` on Option/Result passes the default check again.
