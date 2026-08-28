# Native async FFI 指标报告 / Native async FFI metrics report

## 中文

- 新增 `--ffi-metrics`，在进程退出时向 stderr 输出唯一一条带 `schemaVersion` 的 JSON，不污染业务 stdout。
- 已完成 task 的 queue 与 lifecycle 计数折叠进有界的 module/method 聚合；当前 task 则与 live queue snapshot 合并，避免释放后丢失累计证据或长期保留 capability。
- Host 统一记录 response deadline timeout，以及 cancel 请求、成功与失败；模块内部 retry 与远端耗时明确由模块自身上报。
- JSON 提供 totals 和稳定排序的 module/method rows，覆盖 active/closing/completed task、backlog、oldest age 与所有 queue outcome。
- 单元测试覆盖 deadline timeout 与 cooperative/forced shutdown 后聚合；真实 fswatch 0.0.9 Ctrl-C 验证 `completedTasks=1`、cancel request/success 与 Complete dequeue 均为 1、`forced=0`，指标只出现在 stderr。
- 默认关闭报告时不执行 completed archive 或 outcome 锁更新，且复用已有 task control mutex、不增加每 task 分配；release fibo 五次中位数为 246.086ms，对比上一阶段 250.554ms 无可见回退。

## English

- Add `--ffi-metrics`, which emits exactly one schema-versioned JSON record to stderr at process exit without contaminating business stdout.
- Fold completed-task queue and lifecycle counters into bounded module/method aggregates; merge current tasks with a live queue snapshot so release neither erases cumulative evidence nor retains capabilities indefinitely.
- Count response deadline timeouts and cancellation requests, successes, and failures in the host, while keeping module-internal retries and remote timings module-owned.
- Provide totals and deterministically ordered module/method rows for active/closing/completed tasks, backlog, oldest age, and every queue outcome.
- Cover deadline timeout plus cooperative/forced shutdown aggregation in unit tests; a real fswatch 0.0.9 Ctrl-C run reports `completedTasks=1`, one cancel request/success, one completed dequeue, and `forced=0`, only on stderr.
- When reporting is disabled, skip completed archives and outcome-lock updates, and reuse the existing task-control mutex without adding a per-task allocation. The five-run release fibo median is 246.086ms versus 250.554ms in the previous stage, with no visible regression.
