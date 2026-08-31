---
title: "FFI Interface IR"
summary: "Export deterministic typed FFI contracts for bindgen, inventory, and compatibility checks"
scope: "core"
kind: "spec"
category: "installation"
aliases:
  - "ffi export"
  - "typed ffi bindgen"
  - "interface ir"
entry_for:
  - "calcit ffi export"
id: core/ffi/interface-ir
parent: core/ffi
related:
  - core/ffi
  - core/ffi/async-protocol
---

# FFI Interface IR

`calcit ffi export` reads a project snapshot without evaluating application
code and emits the typed raw-binding boundary as deterministic Interface IR.
It is the stable input planned for Rust, Calcit, TypeScript, and WIT-preview
generators; it does not generate business-level normalization APIs.

```bash
calcit calcit.cirru ffi export
calcit calcit.cirru ffi export --json
calcit calcit.cirru ffi export --json --ns app.ffi
```

The JSON command writes one parseable envelope to stdout. Its
`data.interface` value follows
[`schemas/ffi-interface-ir-v2.schema.json`](../../schemas/ffi-interface-ir-v2.schema.json).
The envelope's `interface_schema` field carries the schema identifier.
Definitions and diagnostics are sorted deterministically, and `revision` is a
digest of the interface plus diagnostics. Unordered EDN maps and sets are
canonicalized recursively before `logical_schema`, `lowering.raw`, and the
revision are emitted. Consumers must check `version` before generation.
`package_version` comes from the adjacent `deps.cirru :version`, which is the
project's release-version source of truth. Legacy projects without that field
fall back to the compatibility version retained in the snapshot.

## Boundary selection

Version 2 selects local definitions whose `:ffi` metadata contains at least
one lowering field: `:backend`, `:target`, `:kind`, `:symbol`, `:invoke`, or
`:transport`. Empty `:ffi {}` placeholders and capability-only metadata such
as `:features` do not declare a raw binding. Malformed non-container metadata
remains visible as a diagnostic instead of disappearing. Dependencies are
excluded; run the command in each module that owns a boundary.

Both layers remain visible:

- `logical_schema` preserves the backend-neutral Calcit schema;
- `signature` is the strict generator-safe projection;
- `declarations` contains only transitively reachable local Struct/Enum shapes;
- `lowering` preserves backend selection and symbol/invocation metadata;
- `status` and `diagnostic_codes` prevent generators from treating an
  unsupported definition as usable.

## Lowering contract validation

Interface IR v2 callable definitions have one direction: Calcit imports a raw
binding from the selected host backend. An explicit import/export direction
field is reserved for a future IR version; consumers must not infer a reverse
export from v2 metadata.

Native callables are generator-safe only when all three lowering fields are
present and coherent:

| Invoke                | Transport          | Symbol |
| --------------------- | ------------------ | ------ |
| `sync`                | `edn-buffer-v1`    | portable C base identifier |
| `async`               | `async-task-v1`    | portable C base identifier |
| `blocking-callback`   | `blocking-host-v1` | portable C base identifier |

The symbol is the unsuffixed logical base such as `read_file`; Calcit derives
the versioned C entry point. Native targets are omitted or `native`. JS targets
are omitted, `browser`, or `node`. Unknown backends, invalid targets, missing
fields, non-portable symbols, unversioned transports, and mismatched
invoke/transport pairs produce path-specific diagnostics before bindgen.

Interface IR v2 的 callable direction 固定为“Calcit 从 host backend import raw
binding”；显式双向 direction 字段留给后续 IR 版本。native callable 必须声明
未带协议后缀的 portable C base symbol，并使用 `sync + edn-buffer-v1`、
`async + async-task-v1` 或 `blocking-callback + blocking-host-v1` 之一。
未知 backend/target、缺失字段、非法 symbol、未版本化 transport 与组合错配都会
在 bindgen 前产生带精确 path 的 diagnostic。

V2 represents `Unit`, `Bool`, `Number`, `String`, `Buffer`, homogeneous
`List`, explicit `Option` / `Result`, and local Struct/Enum references backed
by namespace-qualified declarations. Declaration fields and variant payloads
may use declared type parameters, while callable signatures remain
monomorphic. Only declarations transitively reachable from an FFI signature
enter the document and revision.

Local `defstruct` / `defenum` forms are interpreted statically from the
snapshot; application code is not executed. Missing or ambiguous declarations,
wrong type-argument arity, trait-bounded declarations, `Dynamic`, callbacks,
`Map`, `Set`, `Ref`, resources, host objects, variadic functions, and generic
callable boundaries produce explicit diagnostics. A rejected signature is
`null`; there is no Dynamic fallback or declaration-name guessing.

当前 v2 只导出带有效 lowering 字段的本地 raw binding，忽略 snapshot 中普通
定义的空 `:ffi {}` 占位。Struct/Enum 使用 namespace-qualified declaration ID，
Option/Result 使用明确类型节点；只纳入从 FFI signature 传递可达的声明。缺失、
歧义、参数数量错误或无法表示的声明会产生结构化错误，不会按名称猜测或静默退化
为动态调用。生成器必须先检查 interface `version`、definition `status` 和顶层
`diagnostics`。

`package_version` 读取相邻 `deps.cirru` 的 `:version`，与当前项目发版流程保持
同一事实来源；尚未迁移版本字段的旧项目才回退到 snapshot 兼容值。

## Versioning and scope of v2

The frozen v1 schema remains in the repository for old consumers, but current
exports use v2. A v1-only consumer must reject v2 explicitly and upgrade before
generation; it must not ignore the new `declarations` field and continue with
the old undeclared `named` behavior.

This phase defines an inventory and generator input. It validates fixed
function arity and the published native invocation/transport pairs, but does
not yet validate callback positions, ownership, cancellation, or resource
lifecycle fields nested in lowering metadata. Those structured checks and
generated adapters belong to the next bindgen phase.

## Phase 0 bindgen preview

The repository includes a deliberately narrow preview consumer for measuring
the Interface IR before the generator moves to an independent crate:

```bash
calcit /path/to/calcit.std/calcit.cirru ffi export --json --ns calcit.std.hash > /tmp/calcit-std-ffi.json
node scripts/ffi-bindgen-preview.mjs \
  --input /tmp/calcit-std-ffi.json \
  --out /tmp/calcit-std-bindings
```

For a supported synchronous native `edn-buffer-v1` definition, one v2 input emits
four deterministic previews plus a SHA-256 manifest:

- a Rust typed trait and C-safe adapter stub;
- a Calcit raw wrapper that receives the resolved dylib path;
- a TypeScript declaration;
- a WIT interface/world for the strict monomorphic type subset.

The preview emits Rust and TypeScript Struct/Enum declarations, plus WIT record
and variant declarations for the monomorphic strict subset. It rejects
unsupported definitions, non-native backends, missing symbols,
async/blocking invocation, other transports, missing declarations, and generic
WIT declarations. It does not generate Dynamic fallbacks. Rust adapter bodies
remain explicit
`todo!` stubs because decoder ownership and module implementation binding are
not yet part of Interface IR v2. Resource and async metadata stay visible in
`lowering.raw`, but production generation waits for structured lifecycle
fields.

仓库内提供一个刻意收窄的 Phase 0 preview consumer，用于在拆分独立 crate 前
量化 Interface IR。对于 supported 的同步 native `edn-buffer-v1` definition，
同一输入会确定性生成 Rust typed trait/C-safe adapter stub、Calcit raw wrapper、
TypeScript declaration、Struct/Enum 对应的 Rust/TS/WIT declaration、严格类型
子集的 WIT，以及 SHA-256 manifest。unsupported definition、async/blocking
transport、缺失声明与 WIT 暂不支持的 generic declaration 都会
直接失败，不会生成 Dynamic fallback。Rust decoder body、resource ownership 与
async lifecycle 仍留到结构化 IR 字段和独立 bindgen 阶段完成。
