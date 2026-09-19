# Stackless tail callback / 尾调用 stackless callback

## 中文

- 对“async export 仅原样尾调用同签名 async import”的可证明子集生成 stackless entry 与 callback，不增加 Calcit 表层语法或命令行入口。
- deferred subtask 状态保存在每个 Component task 的 context 中，允许多个任务交错恢复；终态会清空 context，并且精确清理 subtask 与 waitable set。
- 父任务取消会先调用 async `subtask.cancel`；如果取消尚未完成，则重新等待子任务终态，随后只调用一次 `task.cancel`。
- 不能安全证明为直接尾调用的 async export 继续使用现有 stackful lowering。

## English

- Lower the provable subset where an async export directly tail-calls a schema-identical async import into a stackless entry/callback pair, without adding surface syntax or CLI commands.
- Store deferred subtask state in task-local Component context so tasks can resume out of order; clear the context and drop the subtask and waitable set at the terminal transition.
- Propagate parent cancellation through async `subtask.cancel`, wait when cancellation blocks, and call `task.cancel` exactly once after the child reaches a terminal state.
- Preserve the existing stackful lowering for async exports whose control flow is not proven to be a direct tail call.

## Verification / 验证

- `cargo fmt --check`
- `cargo check --all-targets`
- `cargo test --lib component_adapter_rejects`
- `cargo test --test component_wasm_cli`
- `cargo clippy --all-targets -- -D warnings`
