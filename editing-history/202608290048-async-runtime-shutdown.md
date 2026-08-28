# 接入异步 runtime 关闭 / Wire async runtime shutdown

## 中文

- Calcit CLI 统一安装 Ctrl-C handler，信号线程只设置原子状态，`on-control-c` 回调改由 host thread 串行执行。
- 调用 async registry `begin_shutdown`，拒绝未完成 response，调用模块 cancel hook，并给 terminal event 保留 2 秒 grace period。
- grace period 结束后先关闭 host queue，再按 module/method/task/kind/age 输出诊断并强制 purge/release，避免迟到 producer 竞态。
- Watch loop 与兼容性 `async-sleep` 共享可唤醒的 shutdown signal，避免 Ctrl-C 后长时间挂起。
- 添加 response rejection、cancel invocation、grace timeout 强制清理测试，并使用真实 fswatch C-safe Stream 验证 cancel→Complete→release，无强制清理。

## English

- Install one CLI-owned Ctrl-C handler; the signal thread only updates atomic state, while `on-control-c` callbacks run serially on the host thread.
- Invoke async-registry `begin_shutdown`, reject open responses, call module cancel hooks, and retain a two-second grace period for terminal events.
- After the deadline, close the host queue first, then diagnose unfinished tasks by module/method/task/kind/age and force purge/release them without a late-producer race.
- Let watch loops and compatibility `async-sleep` share a wakeable shutdown signal so Ctrl-C cannot leave the process hanging.
- Cover response rejection, cancel invocation, and forced cleanup after grace, plus a real fswatch C-safe Stream cancel→Complete→release smoke with no forced cleanup.
