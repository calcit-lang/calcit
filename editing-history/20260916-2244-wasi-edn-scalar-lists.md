# WASI typed Cirru EDN scalar lists

## 中文

- 让 WASM `try-parse-cirru-edn-as` 直接依据闭合 `DataShapeGraph` 解码顶层同质标量 `List<T>`，没有经过 `Dynamic` 中间值。
- tokenizer 保留 quoted String 内的空白和 escape，并复用已有标量 decoder；输入继续限制为 64 KiB，List 额外限制为 4096 项。
- 语法、item 类型、numeric refinement、tag universe 与 token 超限均返回稳定 `Result :err`，不 trap。
- 用户可观察语义放在 Calcit definition `:tests`；WASM ABI、token budget 与真实执行放在 Wasmtime regression fixture。

## English

- Decode top-level homogeneous scalar `List<T>` values for WASM `try-parse-cirru-edn-as` directly from the closed `DataShapeGraph`, without a `Dynamic` intermediate value.
- Preserve whitespace and escapes inside quoted string tokens and reuse the existing scalar decoder. Input remains bounded at 64 KiB, with an additional 4096-item list limit.
- Return stable `Result :err` values for syntax, item type, numeric refinement, tag-universe, and token-limit failures without trapping.
- Keep user-observable semantics in Calcit definition-attached tests and reserve the Wasmtime regression fixture for WASM ABI, token-budget, and real-execution coverage.
