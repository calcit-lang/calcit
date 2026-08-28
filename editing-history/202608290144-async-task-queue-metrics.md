# 异步 task 队列指标 / Async task queue metrics

## 中文

- 在 async queue 同一把 mutex 下维护有界的 per-task 固定元数据索引，覆盖 queued events、queued bytes、oldest age、accepted、coalesced、queue-full、dequeued 与 purged。
- Enqueue、coalescing、drain 与 purge 的新增指标工作只更新目标 task，不扫描全 registry，也不复制业务 payload；coalescing 延续队列原有的有界定位逻辑。
- `--trace-ffi` 在 enqueue/reject/cancel/release 展示 task-local 指标；shutdown 额外按 module/method 汇总 active/closing tasks 与 backlog。
- Task release 时删除指标状态；forced cleanup 诊断保留最终 backlog 与累计值。
- 单元测试覆盖 coalescing、queue-full、drain、purge、并发 producer 与 shutdown 后指标归零。
- Fibo release 五次中位数由 251.208ms 变为 250.554ms（-0.26%，无可见回退）；真实 fswatch 0.0.9 trace 验证 `Emit → cancel → Complete → release` 且 `forced=0`。

## English

- Maintain a bounded fixed-metadata per-task index under the async queue mutex for queued events, queued bytes, oldest age, accepted, coalesced, queue-full, dequeued, and purged counts.
- The added metric work for enqueue, coalescing, drain, and purge updates only the affected task without scanning the full registry or copying business payloads; coalescing retains the queue's existing bounded lookup.
- Extend `--trace-ffi` enqueue/reject/cancel/release records with task-local metrics and aggregate active/closing tasks plus backlog by module/method during shutdown.
- Remove metric state on task release while retaining the final backlog and cumulative counters in forced-cleanup diagnostics.
- Cover coalescing, queue-full, drain, purge, concurrent producers, and post-shutdown metric removal in tests.
- The five-run release fibo median changes from 251.208ms to 250.554ms (-0.26%, no visible regression); a real fswatch 0.0.9 trace verifies `Emit → cancel → Complete → release` with `forced=0`.
