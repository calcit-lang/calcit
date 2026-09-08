# Strict check-examples schema / 严格 examples runner schema

## Summary / 摘要

- Give the CLI-generated `&calcit:check-examples` runner a structured zero-argument function schema in both native and JavaScript modes. / 为 CLI 生成的 `&calcit:check-examples` runner 在 native 与 JavaScript 模式下提供结构化零参数函数 schema。
- Make the runner finish with canonical `Unit`, so its declared return contract remains exact under zero-debt strict checking. / 让 runner 以规范 `Unit` 结束，使其返回契约在零债务严格检查下保持精确。
- Add focused regression coverage and validate against `calcit.std` examples. / 增加聚焦回归覆盖，并使用 `calcit.std` examples 验证。

## Validation / 验证

- `cargo test examples_schema_is_strict_and_allows_js_ffi`
- Patched CLI against every documented `calcit.std` namespace with `analyze check-examples`. / 使用修复后的 CLI 对所有有文档示例的 `calcit.std` namespace 执行 `analyze check-examples`。
