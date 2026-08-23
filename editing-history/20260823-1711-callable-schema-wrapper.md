# Callable schema wrapper 兼容修复

- 跟进 PR #391 的异步审查，允许零 payload 的 `:: 'Fn` / `:: 'Macro` 进入 canonical symbol parser。
- `Fn` 稳定往返为动态函数 schema；无签名信息的 `Macro` 也解析为动态函数并 canonicalize 为 `Fn`。
- 仅继续拒绝必须携带内部类型的 `Optional`、`JsNullish`、`Variadic` 零 payload 包装。
