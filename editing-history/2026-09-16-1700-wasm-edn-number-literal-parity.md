# WASM Cirru EDN Number 字面量字节一致性

- `format-cirru-edn` 遇到直接 Number 字面量时，在 codegen 阶段复用 `cirru_edn` native formatter 生成常量，避免 WASM 运行时猜测 f64 的类型或重复实现不完整的浮点转文本算法。
- 字符串池收集阶段同步预注册完整格式化文本，保持 emitter 不在布局完成后动态修改 data section。
- Calcit definition `:tests` 覆盖常见正负小数；WASI smoke 直接比较 native 与 Wasmtime 的完整 stdout 字节，再保留关键行断言便于定位。
- 该快路径只覆盖直接 Number 字面量；运行时计算得到的普通 `Number` 仍按 `E_WASM_EDN_TYPE` 失败关闭，后续继续实现完整 f64 路径。
