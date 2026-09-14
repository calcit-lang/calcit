# Typed Component boundary plan

## 中文

- 记录当前 `f64` Calcit value ABI 与 Component Model Canonical ABI 的明确区别。
- 确定 core 拥有 typed directional contract 与 ABI adapter，`calcit-bindgen` 拥有 WIT generation 与 component packaging。
- 收敛到现有 `calcit ffi export --boundary component` 入口，新 contract 以 Cirru EDN 为默认结构化格式。
- 固定严格类型闭包和实施顺序，不为 unsupported value 增加 universal fallback。

## English

- Documented the distinction between the current `f64` Calcit value ABI and the Component Model Canonical ABI.
- Assigned typed directional contracts and ABI adapters to core, while WIT generation and component packaging remain in `calcit-bindgen`.
- Converged on `calcit ffi export --boundary component`; the new contract defaults to Cirru EDN as its structured format.
- Fixed the strict type closure and implementation order without adding a universal fallback for unsupported values.
