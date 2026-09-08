# Check async invocation contracts / 检查异步调用契约

- Treat `:async true` as part of a function's checked invocation contract. The
  declared return type remains the logical value after `js-await`; an invocation
  carries an internal pending boundary until `js-await` unwraps it.
- Preserve the marker through definition references, aliases, callbacks, schema
  serialization, and JS lowering. Async and synchronous callback signatures are
  intentionally incompatible; no automatic await, retry, or general effect model
  is introduced.
- Reject a pending result consumed by an ordinary call with the source-located
  `E_ASYNC_INVOCATION_REQUIRES_AWAIT` diagnostic. Perform this check only after
  argument preprocessing so core bootstrap is not forced to resolve definitions
  early.
- Verified with `cargo test async_ -- --nocapture`, `cargo clippy -- -D warnings`,
  `yarn check-all`, and `bash scripts/check-docs-md.sh`. A current js-ffi fixture
  rejects the missing-await form and emits `await async_value()` for the accepted
  form.

- 将 `:async true` 纳入函数调用契约；`:return` 仍描述 `js-await` 后的逻辑值，
  调用结果在静态检查中先保留为待 await 的内部边界。
- 契约会穿过定义引用、别名、callback、schema 序列化与 JS lowering；
  async/sync callback 不可互换，且不自动插入 await、retry 或扩展通用 effect 模型。
- 普通调用消费未 await 的结果时给出带源码位置的
  `E_ASYNC_INVOCATION_REQUIRES_AWAIT`。检查放在参数预处理之后，避免 core bootstrap
  提前解析尚未就绪的定义。
