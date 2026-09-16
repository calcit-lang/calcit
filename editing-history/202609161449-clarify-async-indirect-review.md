# Clarify async indirect status and test scope / 明确异步间接参数状态与测试范围

## 中文

- indirect async parameter lowering 已完成；后续任务只保留 post-return、stackless callback cancellation 与 stream。
- definition `:tests` 负责可阅读的声明契约，Rust/Node integration 同时验证 raw ABI、内存布局、生命周期、typed `task.return` 与实际结果值。

## English

- Indirect async parameter lowering is complete; only post-return, stackless callback cancellation, and stream remain in the follow-up list.
- Definition-attached tests keep the declaration contract readable, while the Rust/Node integration also validates the raw ABI, memory layout, lifecycle, typed `task.return`, and actual result values.
