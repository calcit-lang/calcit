# Struct index review follow-ups

- Reused native struct index validation for reads, including exact numeric indices and optional field-tag checks.
- Canonicalized paired struct metadata at exported JavaScript constructor boundaries so legacy or external callers cannot create unsorted field/value layouts.
- Aligned JavaScript `withAt` arity validation with the native runtime.
- Added reverse-registration EDN, legacy metadata, stale-tag, fractional-index, and empty-update regressions.
- Re-ran the full Rust, Calcit, JavaScript, IR, WASM, and agent-interface validation suites.
