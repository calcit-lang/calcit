# Isolate the internal async marker / 隔离内部异步标记

- Store the checked async-invocation bit under a private reserved feature tag,
  while continuing to parse and serialize only the public `:async true` schema
  field. This prevents a user-defined `:features #{:async}` flag from silently
  changing invocation or callback semantics.
- 内部调用标记改用不可见的保留 feature tag；公开协议仍只使用 `:async true`。
  这样用户已有的 `:features #{:async}` 不会被误解为异步调用契约。
- Verified with the focused async/schema tests and `cargo clippy -- -D warnings`.
