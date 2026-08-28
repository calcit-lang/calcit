# 类型化 FFI capability / Typed FFI capabilities

## 中文

- 在 core 中增加 nominal `FfiTask` 与 `FfiResponse`，把 native async AnyRef 限制在各自 wrapper 的 `Dynamic` raw 字段中。
- 公开生命周期操作采用方法形式：task 使用 `.cancel` / `.cancel-with`，response 使用 `.resolve` / `.reject`；底层 `&ffi-*` procedure 仅由内部适配函数调用。
- reason 与 payload 使用方法级泛型 `T`。Calcit trait 禁止方法签名包含 `Dynamic`，因此用户值不会在进入 FFI 编码前丢失静态类型。
- 两种 capability 使用不同 nominal receiver；错误 receiver 会被 method dispatch 确定性拒绝，错误 raw capability 仍由 native 宿主校验 kind、owner、generation 与 lifecycle。
- 增加可重放的 architecture scaffold、core 构造测试，以及中英双语协议和使用文档。

## English

- Added nominal core `FfiTask` and `FfiResponse` wrappers, confining native async AnyRef values to each wrapper's `Dynamic` raw field.
- Exposed lifecycle operations as methods: `.cancel` / `.cancel-with` for tasks and `.resolve` / `.reject` for responses. Internal adapters are the only callers of the raw `&ffi-*` procedures.
- Kept reason and payload values typed with method-level generic `T`. Calcit traits reject `Dynamic` method signatures, so caller-side types survive until FFI encoding.
- Kept task and response capabilities as distinct nominal receivers. Method dispatch rejects a wrong receiver deterministically, while the native host still validates raw capability kind, owner, generation, and lifecycle.
- Added a replayable architecture scaffold, core constructor tests, and bilingual protocol and usage documentation.
