# Review: keep JS member kinds outside nominal methods / Review：区分 JS member 与 nominal method

## 中文

- 根据 PR #530 review，将 `W_DYNAMIC_NOMINAL_METHOD_RECEIVER` 严格限制为普通 `MethodKind::Invoke`。
- `.-field` 与 `.!method` 即使命名碰巧是 `unwrap-or`，也属于 JavaScript property/native operation，不应按 Option/Result method 诊断。
- 增加 Access/InvokeNative 反例测试，并将文档中的 `option:*` / `result:*` 明确描述为 Dynamic 边界支持的低层 API；类型化代码仍优先使用 method。
- 目标测试、strict clippy 与格式检查通过。

## English

- Follow PR #530 review by restricting `W_DYNAMIC_NOMINAL_METHOD_RECEIVER` to ordinary `MethodKind::Invoke` calls.
- `.-field` and `.!method` remain JavaScript property/native operations even when their names happen to be `unwrap-or`; they must not be diagnosed as Option/Result methods.
- Add Access/InvokeNative negative regressions and describe `option:*` / `result:*` as supported lower-level Dynamic-boundary APIs while retaining method syntax as the typed-code preference.
- Targeted tests, strict clippy, and formatting checks pass.
