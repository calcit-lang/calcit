# Profile Calx compile cache boundary / 用 profile 固定 Calx 编译缓存边界

## 中文

- 从干净提交 `ef04017f0bc60e2d8f6341599f00d426a11ed360` 采集五个 scalar kernel 的 release 分阶段时间、allocator counters，以及 bounded-simulation 的 1 kHz Samply CPU profile。
- 保存机器、工具链、命令参数、原始 profile 路径/大小/SHA-256、20,987 个 CPU samples、8,018 个 allocator-stack samples 和可复查的 selected stacks；原始 profiler 文件继续留在 `target/`。
- 证据显示 program construction 占分阶段总时间约 47–54%，`emit_expression`、`source_origin` 和 builder emission 主导 CPU 与分配路径。
- 将下一实现边界固定为 embedding-owned、容量有界的 validated-artifact cache；artifact 与每次调用的 host binding attachment 分离，避免 callback/capability state 过期。
- 设计 dependency-validation key：fully-qualified entry、ABI、typed import declaration contract，以及 reachable definitions 的 `DefId`、结构化 `preprocessed_code` 与 schema；无关 definition 变化不失效，transitive code/schema 变化必须 miss。
- 明确当前 `CompiledDef::version_id` 仍为 `0`，不能作为 correctness key；hash 只加速候选查找，最终必须结构比较。
- 增加 profile contract tests，守住干净 commit、五个 kernel、主要 stage、allocator 数据、原始文件哈希与 inclusive-stack 限制。

## English

- Collected release staged timings and allocator counters for all five scalar kernels from clean commit `ef04017f0bc60e2d8f6341599f00d426a11ed360`, plus a 1 kHz Samply CPU profile of bounded-simulation.
- Recorded the machine, toolchain, command parameters, raw-profile paths/sizes/SHA-256 values, 20,987 CPU samples, 8,018 allocator-stack samples, and reviewable selected stacks. Raw profiler files remain under `target/`.
- The evidence shows program construction accounting for approximately 47–54% of staged total time, with `emit_expression`, `source_origin`, and builder emission dominating CPU and allocation paths.
- Froze the next implementation boundary as an embedding-owned, capacity-bounded validated-artifact cache. The artifact is separate from per-call host-binding attachment, preventing stale callbacks or capability state.
- Designed a dependency-validation key covering the fully qualified entry, ABI, typed-import declaration contract, and each reachable definition's `DefId`, structural `preprocessed_code`, and schema. Unrelated definitions preserve hits; transitive code or schema changes must miss.
- Documented that `CompiledDef::version_id` is still `0` and cannot be a correctness key. Hashing may accelerate candidate lookup, but final structural equality remains mandatory.
- Added profile contract tests that preserve the clean commit, five-kernel matrix, dominant stage, allocator evidence, raw-file hashes, and inclusive-stack limitations.
