# Source fix 规划 revision 与审查补强

## 问题

CodeRabbit 指出 `calcit fix` 虽然在规划前读取 Snapshot revision，但调用 staged transaction 时只透传用户可选的 `--expect-revision`。用户未显式传参时，规划与 transaction 首次读取之间存在并发修改窗口；空 operation 分支也没有在返回前检查 expected revision。

## 调整

- staged fix transaction 始终接收规划阶段捕获的 revision，使 preview 与 apply 都绑定同一份 source。
- 空 operation 分支同样校验 expected revision，不把过期空计划报告为当前结果。
- suggestion 增加 `source_file`，与 definition、AST path、fingerprint 一起形成完整 source identity。
- JSON data 增加 staged validation 状态、是否执行 scoped preprocess 以及已检查 operation 数量。
- 增加低层 Rust 测试，验证空计划 revision guard 与同一 source node 的冲突 suggestion 会被拒绝。这两个条件属于 transaction/规划器内部 invariant，无法通过稳定的 Calcit 用户语义可靠构造。

## 保持不变

- 用户可观察的迁移行为继续由 fixture 中的 Calcit definition `:tests` 验证。
- 不为满足通用 docstring 覆盖率统计而增加无助于语义理解的注释；仓库明确不把比例指标作为设计门禁。
