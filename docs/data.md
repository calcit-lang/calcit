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
- **Tag**: Immutable strings starting with `:` (`:keyword`, `:demo`) - similar to Clojure keywords
- **Symbol**: Quoted identifiers such as `'name`; unlike tags, symbols preserve code/data intent
- **String**: Text data with special prefix syntax (`|text`, `"|with spaces"`)
- **Buffer**: Immutable bytes, created with `&buffer`

## Collection Types

- **List**: Ordered persistent collection (`[] 1 2 3`)
- **Map**: Persistent key-value collection (`{} (:a 1) (:b 2)`)
- **Set**: Persistent unordered collection of unique values (`#{} :a :b :c`)

## Named Data

- **Struct**: `defstruct` creates a `StructDef`; `%{}` constructs a fixed-field struct value.
- **Enum**: `defenum` creates an `EnumDef`; `%::` constructs a tagged enum value.
- **Anonymous Struct / Enum**: `%{} _ ...` and `%:: _ ...` create short-lived values without named definitions.
- **Option**: `Option T` has `%some T` and `%none` variants.
- **Result**: `Result T E` has `%ok T` and `%err E` variants.

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

- [String](data/string.md) - String syntax and Tags
- [Persistent Data](data/persistent-data.md) - Implementation details
- [EDN](data/edn.md) - Data notation format
