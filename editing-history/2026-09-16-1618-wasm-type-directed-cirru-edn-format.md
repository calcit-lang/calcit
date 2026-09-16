# WASM 类型导向的 Cirru EDN formatter 起步

- WASM value ABI 以 f64 同时表示 Number、Bool、nil 和 tag id，不能依靠运行时反射恢复可靠的 EDN 标量语义。
- `format-cirru-edn` 首批改为读取预处理后保留的闭合类型，支持 nil、Bool、String、tag、整数 numeric refinement 与递归同质 List；Dynamic、普通 Number 和尚未实现的容器明确返回 `E_WASM_EDN_TYPE`。
- 增加纯 WASM Cirru EDN string leaf formatter，按 Cirru writer 规则在 `|text` 与 `"|quoted text"` 之间选择并处理换行、tab、双引号和反斜杠。
- native definition `:tests` 保存可 review 的共享输出契约；WASI smoke 通过真实 Wasmtime 对照相同字节输出。Rust 测试只覆盖 codegen schema 拒绝与递归深度边界。
- formatter 常量与 tag 文本只在模块实际使用 `format-cirru-edn` 时进入 string pool，避免扩大其他 WASM 模块。
