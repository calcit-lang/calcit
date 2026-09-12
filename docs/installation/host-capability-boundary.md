---
title: "Core, library, and host capability boundary"
summary: "Calcit effect placement rules, backend capability matrix, and review checklist"
scope: "core"
kind: "architecture"
category: "installation"
aliases:
  - "host capabilities"
  - "core effect boundary"
  - "platform capability matrix"
id: core/host-capability-boundary
relates_to:
  - core/ffi
---

# Core, library, and host capability boundary

This document decides where an effectful API belongs and what evidence is
required before it becomes part of Calcit core. Its purpose is to avoid
duplicating behavior between core, Calcit libraries, generated JavaScript, and
native modules.

本文用于判断 effectful API 应位于 core、Calcit library 还是 host adapter，并规定
进入 core 前所需的跨后端证据，避免 core、标准库和 native module 重复实现。

## Ownership rule / 归属规则

Use the narrowest layer that can own the behavior without hiding platform
differences:

| Layer | Owns | Must not own |
| --- | --- | --- |
| Calcit core | Frequent, minimal operations with one stable logical contract; nominal capability values and backend-neutral types | OS policy, retries, process lifecycle, timezone databases, or module-specific business behavior |
| Calcit library | Pure composition, typed methods/wrappers, argument normalization, schema validation, and `Option`/`Result` conversion | Native handles, allocator ownership, raw symbol lookup, or duplicated transport loops |
| Host adapter / native module | Filesystem extensions, process/signal/timer lifecycle, network servers, timezone/crypto engines, and other platform implementations | User-facing normalization that can remain ordinary Calcit code |
| `calcit_native_ffi` | Versioned descriptors, buffers, task/callback transport, ownership helpers, status codes, and backpressure primitives | HTTP, WebSocket, filesystem, process, date, or other domain behavior |
| Macro | Syntax inspection and syntax generation, with declared compile-time read capabilities | Runtime side effects during expansion; host FFI, process, and filesystem writes are forbidden |

选择原则：能在较窄层表达就不要提升到较宽层。纯组合和类型化 wrapper 放在
Calcit library；系统资源与生命周期放在 host adapter；只有高频、最小、逻辑语义
可稳定定义的能力才进入 core。共享 FFI crate 只维护协议，不承载业务功能。

## Core admission criteria / Core 准入标准

A new core effect must satisfy every applicable item:

1. The logical inputs, output, failure model, and cancellation behavior can be
   stated without referring to Rust, Node, DOM, WASI, or a particular library.
2. The API is common enough that depending on a module would create pervasive
   boilerplate rather than a useful capability boundary.
3. Backend differences are either hidden behind the same observable contract
   or rejected explicitly. Silent stubs and placeholder values are forbidden.
4. Expected failures use `Result` or `Option`; raising procedures remain raw
   implementation or compatibility entries, not the preferred application API.
5. Resource ownership and lifecycle are represented by nominal values or typed
   capabilities rather than numbers, strings, or exposed native pointers.
6. The change ships the backend evidence described below and updates this
   matrix. “Implemented in native” alone is not enough to claim cross-backend
   support.

若能力只在单一 backend 有意义，应保留在 adapter/module，并通过 typed library
wrapper 提供体验一致的 API；不要为了名字方便把 platform-specific effect 放进 core。

## Current capability matrix / 当前能力矩阵

“Unavailable” means the implementation must fail at check, codegen, loading, or
invocation with a diagnostic naming the missing capability. It never means a
no-op or fabricated success value.

| Capability | Native eval | Generated JS: Node | Generated JS: browser | Current WASM | Canonical surface |
| --- | --- | --- | --- | --- | --- |
| Pure Calcit data/control and typed `Option`/`Result` composition | yes | yes | yes | supported subset | core definitions and methods |
| JSON parse/stringify | yes | yes | yes | unavailable | String `.parse-json` / Result wrappers; core JSON procedures |
| Current Unix time in milliseconds | yes | unavailable | unavailable | WASI Preview 1 realtime clock; core WASM unavailable | `unix-time-ms` |
| Monotonic milliseconds for elapsed-time measurement | yes | unavailable | unavailable | WASI Preview 1 monotonic clock; core WASM unavailable | `cpu-time`（只比较同一进程内两次调用的差值） |
| 同步等待 | 当前线程 sleep | host injection 或 `Atomics.wait` | 主线程通常不可用并返回错误 | WASI Preview 1 `poll_oneoff`；core WASM unavailable | `wait-ms`，返回 `Result<Unit,String>` |
| Construct and inspect a path value without I/O | yes | yes | yes | value-level support only | `fs:path`, `FsPath .to-string` |
| `FsPath .read-text` / `.write-text` | yes | host injection | browser `localStorage` adapter | WASI Preview 1 preopen；core WASM unavailable | `FsPath` Result-returning methods |
| `FsPath .read-dir` | yes | host injection | unavailable | WASI Preview 1 preopen；core WASM unavailable | `FsPath` Result-returning methods |
| `FsPath .walk-dir` | yes | host injection | unavailable | unavailable | `FsPath` Result-returning methods |
| Process and signal lifecycle | `calcit.std` native module | Node adapter | unavailable | unavailable | typed process/signal APIs in `calcit.std` |
| Repeating timer and timezone/date | `calcit.std` native module | Node adapter | browser adapter | unavailable | typed timer/date APIs in `calcit.std` |
| Glob | focused native module | Node adapter | unavailable | unavailable | focused glob module; do not infer browser support |
| 安全随机字节 | 系统 CSPRNG | Web Crypto | Web Crypto | WASI Preview 1 `random_get`；core WASM unavailable | `secure-random-bytes`，返回 `Result<Buffer,String>` |
| 其他密码学能力 | focused native module | Node adapter | browser crypto adapter | unavailable | focused crypto module or host adapter |
| HTTP fetch | `calcit-fetch` native module | Node adapter | browser adapter | unavailable | typed task/response APIs in `calcit-fetch` or a host adapter |
| HTTP and WebSocket servers | `calcit-http` / `calcit-wss` native modules | Node adapter | unavailable | unavailable | typed server/request/response capabilities in focused modules |
| WebSocket client stream | focused native module | Node adapter | browser adapter | unavailable | typed stream capability in a focused module or host adapter |
| Native dylib sync/async/blocking/resource transport | C-safe FFI v1 | not applicable | not applicable | separate future adapter | raw runtime boundary plus typed library methods |

The matrix records what exists today, not an entitlement for every backend.
该矩阵描述当前事实，不承诺所有能力必须补齐所有后端。新增 API 不得因某个
现存能力已覆盖多个 backend，就绕过跨后端审查与 unsupported diagnostic。

## Canonical core APIs / 当前 core 规范入口

- Open JSON data: String `.parse-json` and its `Result<Dynamic,String>` wrapper;
  decode the Dynamic value into a closed Struct/Enum before business logic.
- Filesystem paths: construct `FsPath` with `fs:path`; use `.read-text`,
  `.write-text`, `.read-dir`, and `.walk-dir`. String-path `try-read-*` and raw
  raising procedures are compatibility or implementation entries.
- 文件系统：WASI command 的 `.read-text` / `.write-text` / `.read-dir` 只解析 host 显式授予的
  preopen，选择最长 guest 路径前缀，并拒绝绝对路径、`..` 越界、非法 UTF-8 与
  无法推进的 partial I/O。`.read-dir` 通过 cookie 分页枚举即时子项、过滤 `.` 与
  `..` 并按完整 guest path 排序；descriptor 生命周期完全留在 adapter 内部。
  递归 `.walk-dir` 仍保持 unavailable。
- 时钟：`unix-time-ms` 返回 Unix epoch 以来的系统时间；`cpu-time` 用于测量
  经过时间，其绝对起点没有跨宿主语义。WASI command 通过 Preview 1
  `clock_time_get` 实现这两个入口，并在宿主返回错误时直接失败，不伪造数值。
  更高层的日期、时区行为仍属于 `calcit.std`。
- 同步等待：`wait-ms` 接收 `0..4294967295` 范围内的整数毫秒，返回
  `Result<Unit,String>`。零值立即成功且不触发宿主调用；小数、负数、溢出和宿主失败
  都进入错误分支，不做隐式舍入。Native 阻塞当前线程，生成的 JavaScript 优先使用
  host injection、否则尝试 `Atomics.wait`；同步 injection 不得返回 Promise/thenable，异步结果会立即转为错误，
  不能先报告成功再丢失稍后的 rejection。WASI command 通过 Preview 1 `poll_oneoff`
  使用相对单调时钟；core WASM 明确拒绝。异步 callback API `timeout-call` 保持独立。
  `async-sleep` 是 fire-and-forget 的兼容任务，单位和生命周期也不同，因此 migration/fix
  不得把这两个旧入口自动改写成 `wait-ms`。类型驱动工具只能报告语义差异并给出两个候选方向：
  需要阻塞顺序时由用户显式改成 `wait-ms`，需要异步调度时保留 callback/task API。
- 安全随机数：`secure-random-bytes` 接收 `0..65536` 范围内的整数长度，返回
  `Result<Buffer,String>`。Native 使用系统 CSPRNG，生成的 JavaScript 使用 Web
  Crypto，WASI command 使用 Preview 1 `random_get`；core WASM 会明确拒绝该宿主能力。
  随机数到领域值的编码、抽样和密码学协议仍放在 Calcit library 或专用模块中。
- Native async/resource values: expose `FfiTask`, `FfiResponse`, and other
  nominal capabilities through methods. Keep raw `&ffi-*`, handles, status
  codes, and symbol strings at module/runtime boundaries.

## Required backend evidence / 新增 effect 的验证要求

A core-effect PR must include:

1. logical contract tests for success, recoverable failure, invalid input, and
   lifecycle/cancellation where relevant;
2. native interpreter tests;
3. generated-JavaScript check/codegen tests and a Node or browser runtime test
   for every backend claimed as supported;
4. a WASM conformance test when supported, or a stable unsupported diagnostic
   test when unavailable;
5. adapter contract tests that reject missing or malformed host injections;
6. documentation updates to the matrix and canonical API list;
7. a real consumer regression when the effect owns external resources or
   long-lived tasks.

Backend output must agree on value shape, error category, ownership, and
exactly-once lifecycle. Transport details may differ. Performance can differ,
but a new adapter should record a baseline when serialization, large payloads,
or high-frequency events are involved.

## Explicit unavailable behavior / 显式不可用行为

- Native dylib calls reject missing protocol/version/method symbols with an FFI
  migration error before invocation; there is no Rust-ABI fallback.
- Generated JavaScript adapters throw a capability-specific error when a
  required host injection or target is absent. They must not return `nil`, an
  empty collection, or success as a substitute.
- WASM codegen must either lower a declared import or reject the unsupported
  operation deterministically. A compiled no-op stub is not acceptable.
- Libraries should preserve these failures as typed `Result` values when the
  failure is expected and recoverable; raw boundary errors may still raise.

## Macro capability versus runtime capability

Macro capabilities describe effects performed while evaluating the expansion,
not effects emitted into runtime code. A pure macro may generate a call to an
effectful runtime function without performing that effect during expansion.

Compile-time reads such as `:env-read`, `:fs-read`, `:platform-read`, and
`:clock-read` must be declared in the Macro schema. `:fs-write`, `:process`, and
`:host-ffi` are rejected during expansion even when declared. Runtime/backend
requirements belong in the generated definition's `:features` / `:ffi`
metadata and adapter contract; they are not macro permissions.

## Library migration and compatibility / 模块迁移建议

When moving behavior between core and a module:

1. land and validate the destination API first;
2. implement the old API as a thin wrapper when compatibility is worthwhile;
3. mark compatibility calls deprecated with an exact replacement and measure
   ecosystem usage before removal;
4. do not keep two native implementations or two retry/lifecycle loops;
5. preserve public value/error semantics during the compatibility window;
6. publish stable module versions before consumers update; use tags, not commit
   hashes;
7. remove the old path only after consumer and backend gates are green.

Small incompatible changes are acceptable during rapid iteration when the
migration diagnostic is deterministic and the replacement is documented.

## Review checklist / 评审清单

- Is the capability in the narrowest correct layer?
- Is the logical contract backend-neutral, or is the backend limitation explicit?
- Are expected absence/failure/cancellation represented in types?
- Are raw handles, symbols, status codes, and allocator details hidden?
- Does every claimed backend have runtime evidence?
- Does every unavailable backend fail explicitly?
- Are macro expansion permissions separated from runtime requirements?
- Is there one implementation of ownership, retry, and lifecycle behavior?
- Are compatibility, deprecation, consumer migration, and release order documented?

See also [Rust bindings](ffi-bindings.md),
[Asynchronous FFI task protocol](ffi-async-protocol.md), and
[FFI upgrade guide](ffi-upgrade-guide.md).
