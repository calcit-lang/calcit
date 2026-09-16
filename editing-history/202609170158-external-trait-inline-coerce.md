# 修复 inline `unsafe-coerce` 的 external-object lowering

## 中文

- 保留 inline `unsafe-coerce` 得到的 external-object trait 类型证据，使普通方法调用继续降级为直接 JavaScript method call。
- 将已声明 external-object 字段上的 `.-field` 读取归一化为 typed external access，从而应用 trait `:names` 映射，而不是读取 Calcit 字段拼写。
- 增加预处理与 JS codegen 回归测试，覆盖 method/field 两条路径。

## English

- Preserve external-object trait evidence from inline `unsafe-coerce` expressions so ordinary method calls lower to direct JavaScript method calls.
- Normalize `.-field` reads for declared external-object fields to typed external access, applying trait `:names` mappings instead of literal Calcit field spellings.
- Add preprocessing and JS codegen regressions for both method and field paths.
