# 隔离 `unsafe-coerce` 预处理测试 / Isolate the `unsafe-coerce` preprocess test

## 中文

- PR #885 的 Linux 测试暴露出两个 `unsafe_coerce` 单元测试之间的共享状态竞争。
- 严格模式测试会临时启用全局 `STRICT_TYPES`，但兼容模式测试未获取 program/preprocess 测试状态锁，因此并行执行时可能错误地产生 `E_UNSCOPED_UNSAFE_COERCE`。
- 让兼容模式测试也持有现有的 `lock_preprocess_test_state`，把全局模式切换与读取纳入同一串行边界；不改变生产代码语义。
- 回归验证应显式以两个线程反复并行运行 `unsafe_coerce` 测试，并执行仓库完整门禁。

## English

- The Linux run for PR #885 exposed a shared-state race between the two `unsafe_coerce` unit tests.
- The strict-mode test temporarily enables the global `STRICT_TYPES` flag, while the compatibility-mode test did not acquire the program/preprocess test-state lock. Parallel execution could therefore produce a spurious `E_UNSCOPED_UNSAFE_COERCE` error.
- The compatibility-mode test now holds the existing `lock_preprocess_test_state`, placing the global mode write and read under the same serialization boundary without changing production semantics.
- Regression validation should repeatedly run both `unsafe_coerce` tests with two test threads and then run the complete repository gates.
