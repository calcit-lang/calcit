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
digest of the interface plus diagnostics. Consumers must check `version`
before generation.

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

## Scope of v1

This phase defines an inventory and generator input. It does not yet validate
every backend-specific arity, transport, ownership, cancellation, or resource
lifecycle rule, and it does not replace the existing C-safe native protocols.
Those checks and generated adapters belong to the next bindgen phase.
