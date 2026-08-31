---
title: "Caps extraction contract"
summary: "Compatibility, version, storage, command, dependency, and regression boundaries for extracting caps from Calcit core"
scope: "tool"
kind: "maintainer-guide"
category: "run"
aliases:
  - "caps repository extraction"
  - "caps compatibility contract"
  - "caps command surface"
  - "拆分 caps"
entry_for:
  - "caps"
  - "deps.cirru"
  - "module-caches"
---

# Caps extraction contract / Caps 拆分契约

## 中文

### 状态与边界

`caps` 是 production package manager，不是 compiler subcommand。它负责解析
`deps.cirru`、Git/SemVer ref、递归依赖图、immutable module store、项目链接、
native artifact realization/receipt/verification，以及相关诊断和恢复。

Calcit core 继续拥有 Snapshot/module loading 语义和 `deps.cirru` 的兼容要求，但不应
长期拥有 resolver、store、Git、lock 或 native artifact 实现。native ABI 的唯一 Rust
source of truth 是 `calcit_native_ffi::abi`；独立 caps 必须用
`default-features = false` 直接消费，不经 `calcit::ffi_abi` 间接引用。

### 当前 extraction inventory

| Source | Scope | Baseline |
| --- | --- | --- |
| `src/bin/calcit_deps.rs` | CLI、manifest/toolchain guard、upgrade/version | 1,267 行，9 项 unit tests |
| `src/bin/caps_graph.rs` | resolver、store、project view、native artifacts、locking/recovery | 1,836 行，15 项 unit tests |
| `src/bin/git/mod.rs` | non-interactive Git clone/ref/status | 93 行 |
| `tests/caps_cli_contract.rs` | 无网络 public CLI contract | help surface、explicit input/version read、missing input failure |

迁移时逐项移动上述实现与 tests；不能复制后让两个仓库长期各自演进。

### 稳定用户契约

| Command | Contract retained across extraction |
| --- | --- |
| default invocation | 解析 root 与 transitive dependencies，materialize immutable revisions，原子安装项目 view |
| `download` / `add` / `remove` | 保持 `owner/repo@ref`、runtime/dev group 与 `deps.cirru` 更新语义 |
| `tree` / `why` | 报告实际 resolved graph、请求来源与最短 root path |
| `version get/set/bump` | 只管理 `deps.cirru :version`，保持 SemVer 校验，不回写 Snapshot |
| `status` / `verify` | 区分 source/link/store/native receipt/toolchain 问题；strong verify 失败返回非零 |
| `reset` / `clean` | 原子重建项目 view；只回收未被项目引用且不是 newest 的 immutable revision |
| `outdated` / `upgrade` | 同时处理 runtime/dev root groups，保留 branch/tag/commit 规则与显式确认 |

所有命令继续接受显式 `<input>`；默认值仍为当前目录的 `deps.cirru`。缺失、解析错误、
冲突 ref、损坏 store/receipt 和 strict warning 必须在 stderr 给出可操作诊断，并以非零
状态退出。stdout 保留给命令结果，供脚本消费。

### 文件与恢复契约

- Global store：`~/.config/calcit/module-caches/`，revision materialize 后不可原地修改；
- Project view：`<project>/.calcit/modules/`，只链接/复制已验证的 store revision；
- State：`<project>/.calcit/caps-state.cirru`；
- 临时内容与目标位于同一文件系统，成功后 rename，失败恢复旧 view/state；
- 每个 store/project metadata mutation 都受 lock 保护；并发 resolve 同一 revision
  只能产生一个有效结果；
- offline 模式可复用已完整 materialize 且 metadata/commit 一致的 cache，不能把半成品
  当作命中；
- native receipt 继续绑定 source commit、build identity、artifact digest 与 ABI protocol。

### 版本解耦

当前 `env!("CARGO_PKG_VERSION")` 同时充当 caps package version 和 expected Calcit
toolchain version，拆仓后必须删除该耦合：

1. `caps --version` 只报告 caps 自身版本；
2. `deps.cirru :calcit-version` 只声明项目需要的 Calcit toolchain；
3. `caps verify --toolchain` 默认探测当前 PATH 中的 `calcit` 并比较版本；
4. CI 或嵌入调用可显式提供已验证的 Calcit 版本，但诊断必须标明来源，不能伪装成
   caps 自身版本；
5. `@calcit/procs` manifest 与 installed version 继续精确匹配 `:calcit-version`。

具体 override flag/API 在独立仓库实现前确定；它不能改变以上三种版本的所有权。

### 迁移回归矩阵

- refs：SemVer tag、`v` tag、branch、full commit、冲突 transitive requests；
- graph：runtime/dev roots、transitive dev exclusion、tree/why、strict warnings；
- storage：fresh install、cache hit、offline hit、lock contention、interrupted materialize、
  project-view rollback、clean preservation；
- native：buffer、async/blocking、resource symbols，receipt digest，allocator ownership；
- CLI：help command surface、显式/默认 input、stdout/stderr/exit status、version get/set/bump；
- real projects：Calcium Workflow 与 Respo 使用稳定 calcit + candidate caps 完成
  install/status/verify/check。

迁移顺序由 [#546](https://github.com/calcit-lang/calcit/issues/546) 索引：先冻结本契约，
再建立独立仓库，发布验证后才从 core 删除实现。

## English

### Status and ownership

`caps` is a production package manager, not a compiler subcommand. It owns
`deps.cirru` resolution, Git/SemVer refs, the recursive graph, immutable module
storage, project views, native artifact realization/receipts/verification, and
their diagnostics and recovery paths.

Calcit core retains Snapshot/module-loading semantics and the compatibility
requirements of `deps.cirru`, but should not continue owning resolver, store,
Git, lock, or native-artifact implementations. `calcit_native_ffi::abi` is the
single Rust source of truth for native ABI. Standalone caps consumes it
directly with default features disabled instead of depending on
`calcit::ffi_abi`.

### Stable contract

Extraction preserves the default install flow and the public
`download/add/remove/tree/why/version/status/verify/reset/clean/outdated/upgrade`
commands. Explicit `<input>` remains supported, with `deps.cirru` in the current
directory as the default. Actionable diagnostics go to stderr and malformed
input, conflicts, corrupt stores/receipts, and strict warnings fail with a
non-zero status; stdout remains available for command results.

The global immutable store remains under
`~/.config/calcit/module-caches/`; project views and state remain under
`<project>/.calcit/`. Materialization and view replacement stay locked,
same-filesystem, atomic, and recoverable. Offline cache hits require complete,
commit-consistent metadata. Native receipts remain bound to source/build
identity, artifact digests, and ABI protocols.

### Version separation

The current `env!("CARGO_PKG_VERSION")` coupling must not cross the repository
boundary. The caps package reports only its own version; `:calcit-version`
selects the project toolchain; toolchain verification probes installed
`calcit` by default; and explicit CI/embedding input reports its provenance.
`@calcit/procs` declarations and installed versions continue matching the
project's Calcit version exactly.

### Regression baseline

Move the existing 24 unit tests and keep coverage for ref selection, graph
groups, storage/cache/locking/recovery, native ABI and receipts, CLI
stdout/stderr/exit behavior, and real Calcium Workflow/Respo installation.
`tests/caps_cli_contract.rs` adds a network-free binary-level baseline for the
public command surface, explicit version reads without mutation, and missing
input failure.

[#546](https://github.com/calcit-lang/calcit/issues/546) indexes the migration:
freeze this contract first, bootstrap the standalone repository second, and
remove the core implementation only after a published candidate passes the
cross-repository smoke.
