---
title: "Data Types"
scope: "core"
kind: "hub"
category: "data"
aliases:
  - "data types"
  - "persistent data"
  - "immutable data"
id: core/data
---
# Data Types

Calcit uses persistent values by default, with a small set of explicit stateful containers. The same logical values are available in the Rust and JavaScript runtimes; WASM supports a growing, documented subset.

## Primitive Types

- **Bool**: `true`, `false`
- **Nil**: `nil`, used for absence at untyped boundaries
- **Number**: `f64` in Rust, Number in JavaScript (`1`, `3.14`, `-42`)
- **Tag**: Interned immutable identifiers starting with `:` (`:keyword`, `:demo`), commonly used for field names, variants, and protocol labels
- **Symbol**: Quoted identifiers such as `'name`; unlike tags, symbols preserve code/data intent
- **String**: Text data with special prefix syntax (`|text`, `"|with spaces"`)
- **Buffer**: Immutable bytes, created with `&buffer`

## Collection Types

- **List**: Ordered persistent collection (`[] 1 2 3`)
- **Map**: Persistent key-value collection (`{} (:a 1) (:b 2)`)
- **Set**: Persistent unordered collection of unique values (`#{} :a :b :c`)

## Named Data

- **Struct**：`defstruct` 创建 `StructDef`；具名定义可直接用 `Struct :field value` 构造。
- **Enum**：`defenum` 创建 `EnumDef`；具名定义可直接用 `Enum :variant value` 构造。
- **匿名 Struct / Enum**：`%{} _ ...` 和 `%:: _ ...` 创建没有具名定义的临时值。
- **Option**：`Option<T>` 用 `Option :some value` 或 `Option :none` 构造。
- **Result**：`Result<T,E>` 用 `Result :ok value` 或 `Result :err error` 构造。

Prefer named structs and enums at module boundaries: their definitions carry schemas, support trait attachment, and give static analysis more information than ad-hoc maps or anonymous values.

## Explicitly Stateful Values

- **Ref**: Mutable reference cell used for controlled application state.
- **BufList**: Mutable builder for allocation-sensitive loops; convert it to a persistent List before exposing the result.
- **AnyRef**: Opaque host reference for FFI. It is not portable serialized data.

## Executable Values

- **Function**: User-defined functions and built-in procedures
- **Proc**: Internal procedure type for built-in functions
- **Trait / Impl**: Runtime capability descriptors used for method dispatch

## Implementation Details

- **Rust runtime**: Uses [rpds](https://github.com/orium/rpds) for HashMap/HashSet and [ternary-tree](https://github.com/calcit-lang/ternary-tree.rs/) for vectors
- **JavaScript runtime**: Uses [ternary-tree.ts](https://github.com/calcit-lang/ternary-tree.ts) for all collections

Collection method availability is also expressed through built-in traits. `Countable` and `Contains` cover List, Map, Set, String, Struct, and Enum; `Compare` covers Number and String. See [Polymorphism](features/polymorphism.md) for the full matrix.

For serialization fidelity and unsupported runtime values, see:

- [Number](data/number.md) - 数值运算的跨后端语义（取余 `rem`）
- [String](data/string.md) - String syntax and Tags
- [Persistent Data](data/persistent-data.md) - Implementation details
- [EDN](data/edn.md) - Data notation format
