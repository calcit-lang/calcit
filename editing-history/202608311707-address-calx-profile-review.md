# 2026-08-31 17:07 Calx compile profile review follow-up

## 中文

- compile profile 不再把“所有编译完成”写成 `correctness: true`，改为语义准确的
  `compilationSucceeded`；kernel 执行正确性继续由默认 benchmark 与 core differential corpus 负责。
- allocation window 通过进程内 mutex 串行化，并用 RAII 在错误或 panic unwind 时关闭全局计数开关，避免
  重叠或异常路径污染后续采样。
- revision-safe cache 契约增加有界 recently-evicted slot-key ledger，明确 `empty`/`evicted` 判定、
  reinsertion、clear、统计与测试序列。

## English

- Compile-profile output now reports `compilationSucceeded` instead of claiming unexecuted kernels have
  `correctness: true`; execution correctness remains owned by the default benchmark and core differential corpus.
- The allocation window is serialized by a process-local mutex and uses RAII to disable global counting after errors
  or panic unwinding, preventing overlapping or abandoned measurements from contaminating later samples.
- The revision-safe cache contract now specifies a bounded recently-evicted slot-key ledger, including
  `empty`/`evicted` classification, reinsertion, clear, statistics, and required test sequences.
