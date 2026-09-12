# WASI 临时内存复用 / Reusable WASI scratch memory

## 中文

- 将 WASI 参数与环境变量适配器的宿主原始缓冲区移出 Calcit 托管 bump heap，改为从线性内存顶部按调用复用的临时区域。
- 在预留临时区域时同时估算本次适配器仍会产生的托管对象空间，必要时通过 `memory.grow` 扩容，避免字符串与列表分配覆盖宿主缓冲区。
- 使用 64 位中间值检查 Preview 1 返回的数量和缓冲区大小，并验证参数指针有序、互不重叠且以 NUL 结尾。

验证：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`bash scripts/test-wasi-preprocess.sh`。

## English

- Moved raw host buffers used by the WASI arguments and environment adapters out of the Calcit managed bump heap into a per-call scratch region reused from the top of linear memory.
- Reserve enough room for managed objects allocated while the adapter is active and grow memory when needed, preventing strings and lists from overlapping the host buffer.
- Use 64-bit intermediate sizes for Preview 1 counts and buffers, and validate that argument pointers are ordered, non-overlapping, and NUL-terminated.

Verified with `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `bash scripts/test-wasi-preprocess.sh`.
