# 修复 WASM Cirru EDN 整数格式化返回类型

- `__rt_f64_to_str` 已经返回 WASM `f64` 形式的 Calcit 字符串指针，调用方不应再次执行 `F64ConvertI32U`。
- 在 Calcit definition `:tests` 与 Wasmtime smoke 中加入整数格式化，确保该 lowering 路径会被真正实例化和执行。
- 复核了字符串反斜杠评论：当前 `cirru_parser` writer 的 `ALLOWED_CHARS` 明确包含反斜杠，native `format-cirru-edn` 也保持裸 leaf，因此不改变 WASM runtime 的对应规则。
