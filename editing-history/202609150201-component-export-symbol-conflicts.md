# Component export symbol conflicts

- Component adapter 在 WASM 编码前按 definition 排序，并检查所有 `defwasm-export` 的公开 symbol。
- 跨 namespace 的重复 symbol 现在以稳定的 `E_COMPONENT_ABI_SYMBOL_CONFLICT` 失败，并列出排序后的 owner definitions，避免把错误推迟到 WASM validator。
