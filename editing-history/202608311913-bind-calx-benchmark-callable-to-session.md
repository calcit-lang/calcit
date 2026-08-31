# 将 Calx benchmark callable 绑定到 session

## 中文

- `CalxBenchmarkCalcitCallable` 现在携带创建它的 `CalxBenchmarkSession` 借用，Rust 生命周期会阻止 callable 脱离 session 存活。
- cached callable 每次执行都通过原 session 读取固定 definition contract；session guard 在 callable 存活期间保持当前 corpus 的进程级 runtime ownership。
- 这避免后续 benchmark session 替换 namespace 后，旧 callable 继续对新的 runtime cells 执行。

## English

- `CalxBenchmarkCalcitCallable` now borrows the `CalxBenchmarkSession` that resolved it, so Rust lifetimes prevent the callable from outliving or detaching from that session.
- Every cached invocation reads its fixed definition contract through the originating session; the session guard retains process-level runtime ownership for the corpus while the callable is alive.
- This prevents an old callable from executing against replacement runtime cells after a later benchmark session installs another namespace corpus.
