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
[`schemas/ffi-interface-ir-v1.schema.json`](../../schemas/ffi-interface-ir-v1.schema.json).
The envelope's `interface_schema` field carries the schema identifier.
Definitions and diagnostics are sorted deterministically, and `revision` is a
digest of the interface plus diagnostics. Unordered EDN maps and sets are
canonicalized recursively before `logical_schema`, `lowering.raw`, and the
revision are emitted. Consumers must check `version` before generation.
`package_version` comes from the adjacent `deps.cirru :version`, which is the
project's release-version source of truth. Legacy projects without that field
fall back to the compatibility version retained in the snapshot.

## Boundary selection

Version 1 selects local definitions whose `:ffi` metadata contains at least
one lowering field: `:backend`, `:target`, `:kind`, `:symbol`, `:invoke`, or
`:transport`. Empty `:ffi {}` placeholders and capability-only metadata such
as `:features` do not declare a raw binding. Malformed non-container metadata
remains visible as a diagnostic instead of disappearing. Dependencies are
excluded; run the command in each module that owns a boundary.

Both layers remain visible:

- `logical_schema` preserves the backend-neutral Calcit schema;
- `signature` is the strict generator-safe projection;
- `lowering` preserves backend selection and symbol/invocation metadata;
- `status` and `diagnostic_codes` prevent generators from treating an
  unsupported definition as usable.

V1 represents `Unit`, `Bool`, `Number`, `String`, `Buffer`, homogeneous
`List`, and nominal named types (including representable `Option`, `Result`,
struct, and enum references). `Dynamic`, callbacks, `Map`, `Set`, `Ref`, host
objects, variadic functions, and generic callable boundaries produce explicit
diagnostics. A rejected signature is `null`; there is no Dynamic fallback.

当前 v1 只导出带有效 lowering 字段的本地 raw binding，忽略 snapshot 中普通
定义的空 `:ffi {}` 占位。JSON 输出保持确定性，并对 `Dynamic`、callback、
Map/Set、Ref、host object、可变参数或泛型调用边界给出结构化错误，不会静默
退化为动态调用。生成器必须先检查 interface `version`、definition `status`
和顶层 `diagnostics`。

`package_version` 读取相邻 `deps.cirru` 的 `:version`，与当前项目发版流程保持
同一事实来源；尚未迁移版本字段的旧项目才回退到 snapshot 兼容值。

## Scope of v1

This phase defines an inventory and generator input. It does not yet validate
every backend-specific arity, transport, ownership, cancellation, or resource
lifecycle rule, and it does not replace the existing C-safe native protocols.
Those checks and generated adapters belong to the next bindgen phase.

## Phase 0 bindgen preview

The repository includes a deliberately narrow preview consumer for measuring
the Interface IR before the generator moves to an independent crate:

```bash
calcit /path/to/calcit.std/calcit.cirru ffi export --json --ns calcit.std.hash > /tmp/calcit-std-ffi.json
node scripts/ffi-bindgen-preview.mjs \
  --input /tmp/calcit-std-ffi.json \
  --out /tmp/calcit-std-bindings
```

For a supported synchronous native `edn-buffer-v1` definition, one input emits
four deterministic previews plus a SHA-256 manifest:

- a Rust typed trait and C-safe adapter stub;
- a Calcit raw wrapper that receives the resolved dylib path;
- a TypeScript declaration;
- a WIT interface/world for the strict primitive/List subset.

The preview rejects unsupported definitions, non-native backends, missing
symbols, async/blocking invocation, other transports, and WIT named types. It
does not generate Dynamic fallbacks. Rust adapter bodies remain explicit
`todo!` stubs because decoder ownership and module implementation binding are
not yet part of Interface IR v1. Resource and async metadata stay visible in
`lowering.raw`, but production generation waits for structured lifecycle
fields.

仓库内提供一个刻意收窄的 Phase 0 preview consumer，用于在拆分独立 crate 前
量化 Interface IR。对于 supported 的同步 native `edn-buffer-v1` definition，
同一输入会确定性生成 Rust typed trait/C-safe adapter stub、Calcit raw wrapper、
TypeScript declaration、严格 primitive/List 子集的 WIT，以及 SHA-256 manifest。
unsupported definition、async/blocking transport 与未声明的 WIT named type 都会
直接失败，不会生成 Dynamic fallback。Rust decoder body、resource ownership 与
async lifecycle 仍留到结构化 IR 字段和独立 bindgen 阶段完成。
