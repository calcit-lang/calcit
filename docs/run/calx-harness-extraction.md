# Calx benchmark harness extraction contract / Calx 基准工具拆分契约

## Status / 状态

This document freezes the phase-one boundary tracked by
[calcit#557](https://github.com/calcit-lang/calcit/issues/557). The harness is
an **experimental benchmark/research product**, not a Calcit runtime feature,
language correctness gate, or production dependency. This phase documents the
contract and bootstrap inventory; it does not create a repository or remove
assets from Calcit core.

本文冻结 [calcit#557](https://github.com/calcit-lang/calcit/issues/557)
追踪的第一阶段边界。该 harness 是**实验性 benchmark/research 产品**，不是 Calcit
runtime 功能、语言正确性 gate 或生产依赖。本阶段只记录契约和 bootstrap inventory，
不创建仓库，也不从 Calcit core 删除资产。

The machine-readable bootstrap manifest is
[`calx-harness-bootstrap.json`](./calx-harness-bootstrap.json). Issue
[calcit#547](https://github.com/calcit-lang/calcit/issues/547) is the product
tracker; [calcit#558](https://github.com/calcit-lang/calcit/issues/558) owns the
standalone bootstrap, and [calcit#559](https://github.com/calcit-lang/calcit/issues/559)
owns the later core cutover.

## Product and repository boundary / 产品与仓库边界

The proposed standalone repository name is `calcit-lang/calcit-calx-bench`,
pending maintainer confirmation in #558. The existing
[`calcit-lang/calcit-calx`](https://github.com/calcit-lang/calcit-calx) remains
a native FFI demo and must not silently absorb benchmark orchestration,
historical reports, or release policy.

建议的独立仓库名为 `calcit-lang/calcit-calx-bench`，等待维护者在 #558 确认。已有
[`calcit-lang/calcit-calx`](https://github.com/calcit-lang/calcit-calx)
继续保持 native FFI demo 定位，不静默承接 benchmark orchestration、历史报告或发布策略。

Calcit core and `calx-vm` remain the source of truth for lowering, eligibility,
typed boundaries, VM semantics, and differential correctness. The standalone
harness owns experiment settings, process orchestration, report aggregation,
raw sample preservation, methodology, and archived measurements. It has no
business version cadence; releases identify harness/schema changes, while each
measurement pins exact Calcit and `calx-vm` revisions.

Calcit core 与 `calx-vm` 继续作为 lowering、eligibility、typed boundary、VM
语义和 differential correctness 的 source of truth。独立 harness 负责实验参数、进程调度、
报告聚合、原始样本保留、方法文档和历史测量。它不按业务功能迭代版本；release 只标识
harness/schema 变化，每次测量必须固定精确的 Calcit 与 `calx-vm` revision。

## Asset ownership and migration / 资产归属与迁移

| Asset | Phase-two action | Long-term owner | Reason |
| --- | --- | --- | --- |
| `src/bin/calx_bench.rs` | migrate, then replace with the narrow adapter consumer | standalone harness | single-case runner and measurement phases |
| `scripts/bench-calx-e2e.mjs` | migrate | standalone harness | process/profile/sample orchestration and aggregation |
| `scripts/bench-calx-settings.mjs` and test | migrate | standalone harness | experiment-setting policy |
| `docs/run/calx-benchmark.md` | migrate methodology; leave a short core link | standalone harness | measurement and comparison policy |
| `benchmarks/calx/README.md` and versioned JSON | migrate without rewriting raw data | standalone harness | archive and bounded conclusions |
| `tests/fixtures/calx/scalar-kernels.cirru` | copy with source revision and keep original | both, core authoritative | benchmark workload also serves differential correctness |
| `src/codegen/calx.rs`, `src/codegen/calx/lowering.rs` | stay | Calcit core | backend semantics and typed boundary |
| `src/program/tests.rs` Calx tests | stay | Calcit core | eligibility, golden, trap, fallback, and differential correctness |
| all `tests/fixtures/calx/*.golden.txt`, `fallback.cirru`, `typed-imports.cirru` | stay | Calcit core | language/backend correctness, not benchmark policy |
| `Cargo.toml` binary entry and `package.json` benchmark scripts/check | remove only in #559 | Calcit core during migration | cut over only after standalone reproduction succeeds |

The scalar source fixture is deliberately the only shared/copy-with-provenance
asset. The standalone copy records its originating Calcit commit and must not
become the correctness source of truth. Golden and fallback fixtures do not
move.

scalar source fixture 是唯一明确允许 copy-with-provenance 的共享资产。独立仓库副本必须记录
来源 Calcit commit，不能成为 correctness source of truth；golden 与 fallback fixtures 不迁移。

## Frozen report and process contract / 固化的报告与进程契约

- A successful single-case runner writes exactly one JSON value to stdout with
  schema `calcit-calx-benchmark/2`; progress and diagnostics never share stdout.
- A successful suite writes schema `calcit-calx-benchmark-suite/2` and preserves
  every `rawSamples` entry in addition to median and median absolute deviation.
- The core transition adds opt-in `calcit-calx-compile-profile/1` and
  `calcit-calx-cache-profile/1` single-case evidence. Both schemas and their
  commands migrate with the runner; they do not extend the archived suite-v2
  objects in place.
- Parse, build, correctness, schema, and runtime failures go to stderr and exit
  nonzero. Partial JSON is never reported as success.
- Reports retain debug/release profile, OS/architecture/CPU/memory, complete
  Rust/Cargo/Node identity, exact Calcit commit plus dirty state, resolved
  `calx-vm` version, workload/matrix/settings, warm-up policy, and scope gaps.
- Archived raw reports are immutable. A schema change creates a new schema ID
  and migration note rather than rewriting old files.
- Ratios and crossover points are informational. Machine-specific absolute
  thresholds do not enter ordinary Calcit correctness CI.

- 单 case 成功时 stdout 只写一个 `calcit-calx-benchmark/2` JSON；进度和诊断不混入 stdout。
- suite 成功时写 `calcit-calx-benchmark-suite/2`，在 median 与 MAD 之外保留全部
  `rawSamples`。
- core 迁移期新增 opt-in 的 `calcit-calx-compile-profile/1` 与
  `calcit-calx-cache-profile/1` 单 case 证据；schema 与命令随 runner 一起迁移，不原地扩展历史 suite-v2。
- parse/build/correctness/schema/runtime 失败写 stderr 并非零退出，不把 partial JSON 当成功结果。
- 报告保留 profile、主机/工具链、精确 Calcit commit 与 dirty 状态、resolved `calx-vm`
  版本、workload/matrix/settings、预热政策和未覆盖范围。
- 已归档 raw report 不可改写；schema 变化使用新 ID 和 migration note。
- ratio/crossover 只提供信息，不把机器相关绝对阈值加入普通 correctness CI。

## Revision-pinned internal adapter / 固定 revision 的内部 adapter

The standalone runner must stop reaching `PROGRAM_CODE_DATA`,
`ProgramFileData`, `ensure_def_id`, `run_fn`, or mutable global registries
directly. A narrow adapter compiled from the pinned Calcit revision exposes one
session-oriented path:

1. install a named source corpus with explicit fixed function schemas;
2. preprocess once and return an immutable benchmark program handle;
3. resolve one cached Calcit callable from that handle;
4. compile one measured Calx kernel from the same handle;
5. execute prepared Calcit arguments and strict Calx values through the two
   cached call paths;
6. return stage timings, stable program counts, and correctness values without
   exposing mutable registries.

独立 runner 不再直接访问 `PROGRAM_CODE_DATA`、`ProgramFileData`、`ensure_def_id`、
`run_fn` 或可变全局 registry。由固定 Calcit revision 编译的窄 adapter 只暴露 session
路径：安装带显式 schema 的 source corpus、一次 preprocess 得到 immutable handle、从同一
handle 解析 cached Calcit callable 和 measured Calx kernel、执行预先准备的两条调用路径，并返回
阶段 timing、稳定 program counts 和 correctness value。

This adapter is an **internal benchmark API**. It has no semver compatibility
promise and must not become a general embedding API by accident. The harness
pins a Calcit commit/tag, upgrades deliberately, and runs its compile and
quick-smoke matrix before changing the pin. Mutable-global access, implicit
source installation, automatic fallback after effects, and benchmark policy
inside core are forbidden.

该 adapter 是 **internal benchmark API**，不承诺 semver 兼容，也不能意外扩张成通用
embedding API。harness 固定 Calcit commit/tag，升级 pin 前运行 compile 与 quick-smoke matrix。
禁止暴露可变全局、隐式安装源码、effect 后自动 fallback，以及把 benchmark policy 放回 core。

## Test and smoke migration matrix / 测试与 smoke 迁移矩阵

| Current coverage | Phase-two destination | Core after cutover |
| --- | --- | --- |
| Rust: resolved `calx_vm` lockfile identity | standalone runner test | dependency/build correctness remains |
| Rust: duplicate/missing dependency identity rejection | standalone runner test | not required by language semantics |
| Rust: cached callable equals lookup execution and missing entry fails | adapter integration test | differential execution remains in `src/program/tests.rs` |
| Node: absent setting uses fallback | standalone settings test | removed from `check-all` in #559 |
| Node: complete safe integers accepted | standalone settings test | removed from `check-all` in #559 |
| Node: partial/fractional/exponential/padded values rejected | standalone settings test | removed from `check-all` in #559 |
| Node: unsafe/below-minimum values rejected | standalone settings test | removed from `check-all` in #559 |
| `CALX_BENCH_QUICK=1 CALX_BENCH_SAMPLES=1 yarn bench-calx-e2e` | required PR smoke | core keeps it until standalone passes |
| full debug/release matrix | manual/versioned evidence workflow | not a correctness gate |

The phase-two bootstrap is accepted only when the standalone harness builds
against an explicit Calcit revision, passes all seven migrated tests, emits a
schema-v2 quick report with raw samples, and links its README/AGENTS status and
boundaries back to #547/#558. Only then may #559 remove the core binary and
policy checks.

第二阶段只有在独立 harness 固定 Calcit revision、通过迁移的 3 Rust + 4 Node tests、输出保留
raw samples 的 schema-v2 quick report，并在 README/AGENTS 回链 #547/#558 后才通过验收；之后
#559 才能删除 core binary 和 policy checks。
