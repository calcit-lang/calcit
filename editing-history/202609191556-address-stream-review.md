# 收紧流边界并处理 review / Tighten stream boundaries and address review

## 中文

- 明确文档中的流上限必须是正整数字面量，且 chunk 上限不能超过总量上限。
- 在预处理阶段拒绝通过公开或 raw Struct 构造器创建 `ReadableByteStream`，保留它作为仅由 Component host 提供的 opaque handle marker。
- 将异常的 zero-count completed read 收敛为终态空 read，避免 guest 在一次调用内忙循环，并增加 exactly-once cleanup 回归覆盖。
- 记录并断言 `stream-cancel-read` 恰好调用一次，补强主动取消生命周期测试。

## English

- State explicitly that stream limits are positive integer literals and that the chunk limit cannot exceed the total limit.
- Reject public and raw Struct construction of `ReadableByteStream` during preprocessing, preserving it as an opaque handle marker supplied only by the Component host.
- Terminate a malformed zero-count completed read as an empty terminal read so the guest cannot busy-loop inside one call, with exactly-once cleanup coverage.
- Count and assert exactly one `stream-cancel-read` call in the active-cancellation lifecycle test.
