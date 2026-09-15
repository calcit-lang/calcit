# Component Struct nested container coverage

## 中文

- 扩展 Component Struct 集成 fixture，在 record 字段中覆盖 Option<String> 与 Result<List<Number>, String>。
- 在 Calcit definition `:tests`、direct export 和 imported-host 往返中验证嵌套容器，供 calcit-bindgen 的 Wasmtime/jco 端到端验证消费。

## English

- Extended the Component Struct integration fixture with Option<String> and Result<List<Number>, String> record fields.
- Verified the nested containers through Calcit definition `:tests`, direct exports, and imported-host round trips for calcit-bindgen's Wasmtime/jco end-to-end coverage.
