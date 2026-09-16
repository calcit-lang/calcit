# WASI Cirru EDN nominal Struct roundtrip

## 中文

- WASM/WASI 的 `try-parse-cirru-edn-as` 现在直接消费已有 `DataShapeGraph`，支持顶层 nominal Struct 与闭合标量字段，不引入运行时类型探测。
- 解析器校验 canonical `%{} 'TypeName (:field value)` 的类型名、完整字段集合、重复字段和字段类型；失败继续通过 `Result :err` 表达。
- `format-cirru-edn` 根据推导出的 nominal schema 和声明字段顺序生成与 native 一致的 Cirru EDN 字节。
- 用户语义放在 Calcit definition `:tests`，Rust 测试只覆盖 shape handle 到 WASM tag universe 的编译器内部契约；Wasmtime smoke 覆盖真实 command lowering。
- 嵌套 Struct、容器字段和 Enum 仍 fail closed，留给后续递归 formatter/parser 切片。

## English

- WASM/WASI `try-parse-cirru-edn-as` now consumes the existing `DataShapeGraph` for top-level nominal Struct values with closed scalar fields, without runtime type probing.
- The parser validates the canonical `%{} 'TypeName (:field value)` name, exact field set, duplicates, and field types; failures remain explicit `Result :err` values.
- `format-cirru-edn` uses the inferred nominal schema and declaration field order to emit native-compatible Cirru EDN bytes.
- User-visible semantics live in Calcit definition `:tests`; the Rust test only covers the internal shape-handle-to-WASM-tag-universe contract, while the Wasmtime smoke covers real command lowering.
- Nested Struct values, container fields, and Enum values still fail closed for later recursive formatter/parser slices.
