# 避免 shutdown 重复取消 / Avoid duplicate shutdown cancellation

## 中文

- 在 `begin_shutdown` 之前保留 lifecycle snapshot，只向原先处于 `Active` 的 task 调用 cancel hook。
- 如果 `on-control-c` 或业务逻辑已经把 task 转为 `Closing`，全局 shutdown 只等待 terminal，不会重复调用模块取消。
- 测试断言已在 Closing 的 task 取消次数保持为零，同时继续受 grace timeout 和强制清理保护。

## English

- Preserve the lifecycle snapshot before `begin_shutdown` and invoke cancel hooks only for tasks that were originally `Active`.
- If `on-control-c` or application logic already moved a task to `Closing`, global shutdown waits for its terminal acknowledgement without calling module cancellation twice.
- Assert that an already-closing task receives zero additional cancel calls while remaining protected by the grace timeout and forced cleanup.
