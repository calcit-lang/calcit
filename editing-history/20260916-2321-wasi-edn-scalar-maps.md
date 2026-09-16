# WASI typed Cirru EDN scalar maps

## 中文

- 让 WASM `try-parse-cirru-edn-as` 直接依据闭合 `DataShapeGraph` 解码顶层同质标量 `Map<K,V>`，没有经过 `Dynamic` 中间值。
- parser 复用标量 decoder，保留 quoted String 的空白与 escape；输入继续限制为 64 KiB，Map 额外限制为 2048 项。
- 修正 WASM HAMT 的 String key 语义：运行时生成的等值字符串现在按 UTF-8 内容 hash 和比较，不再依赖分配地址。
- 补齐先前 scalar parser review 遗留：扫描可内联 top-level value、完整收集 parse-only literals，并在 f64 转换前拒绝整数 refinement 的非零小数位。
- 用户可观察语义放在 Calcit definition `:tests`；WASM ABI、资源上限与真实执行放在 Wasmtime regression fixture。

## English

- Decode top-level homogeneous scalar `Map<K,V>` values for WASM `try-parse-cirru-edn-as` directly from the closed `DataShapeGraph`, without a `Dynamic` intermediate value.
- Reuse the scalar decoder while preserving whitespace and escapes in quoted strings. Input remains bounded at 64 KiB, with an additional 2048-entry map limit.
- Fix WASM HAMT String-key semantics so equal runtime-created strings hash and compare by UTF-8 content rather than allocation identity.
- Close earlier scalar-parser review gaps by scanning inlineable top-level values, interning every parse-only literal, and rejecting non-zero fractional digits for integer refinements before f64 conversion.
- Keep user-observable semantics in Calcit definition-attached tests and reserve the Wasmtime regression fixture for WASM ABI, resource-limit, and real-execution coverage.
