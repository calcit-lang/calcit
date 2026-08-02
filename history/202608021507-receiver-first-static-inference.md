# Receiver-first calls with stronger static inference

- Extended receiver-first rewriting from Result/Option to every receiver whose
  static method table is known, including built-in collections, primitives,
  traits, structs, records, and enums. Prefix calls remain compatible.
- Preserved concrete `deftrait`, `defimpl`, `defstruct`, and `defenum` metadata
  through local bindings and imported definitions, so `impl-traits` no longer
  collapses nominal values to `Dynamic` or a broad `Custom` type.
- Propagated generic `:where` capabilities into function bodies and enum
  `match` payloads, including lexical data definitions and nested named type
  references. `%::` now retains enum identity even when a payload-free variant
  cannot determine every generic argument.
- Inferred typed trait-method returns, required struct fields read through
  `get`, and body-hinted parameter types. Broad `assert-type` checks no longer
  erase a more precise compatible inferred type.
- Migrated representative Calcit snapshots and Markdown examples to
  receiver-first syntax, formatted every touched Cirru block, and executed the
  documented outputs on native and JavaScript targets.
- Kept conservative `Dynamic` fallback for opaque FFI values, heterogeneous or
  genuinely unresolved nested data, and metadata that cannot be resolved
  without executing arbitrary user code.
