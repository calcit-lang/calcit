# Macro compile-time capabilities

Issue #435 adds an enforceable effect boundary around strict macro execution.
The capability policy is centralized in `runner::macro_capability` and remains
active through ordinary helper calls by using a scoped evaluator context.

Strict `MacroSignature` values now serialize a dedicated `:capabilities`
hashset. Pure expansion needs no declaration. Environment, filesystem,
platform, clock, mutable-state, and dynamic-eval reads/actions may be declared;
filesystem writes, process control, and host FFI are classified but always
rejected. Legacy macro signatures stay compatible, but their effects remain
unknown and they are not eligible for future pure-expansion caching.

The evaluator checks builtin procedures, state/dynamic-eval syntax, native
methods, registered procedures, and raw host code. Missing declarations use
`E_MACRO_CAPABILITY_MISSING`; forbidden operations use
`E_MACRO_CAPABILITY_DISALLOWED`. Both preserve the macro call location and
helper chain. Quoted code that performs an effect later at runtime remains a
pure expansion. Macro diagnostics also reuse the raw call-head location instead
of the locationless resolved import, preserving no-argument call sites.

Validation covered formatting, clippy, all Rust and CLI tests, native/JS/IR/WASM
integration, agent-interface checks, and the macro-signature documentation.
Latest Respo passed 27 tests and 126 documentation blocks. Latest Recollect
passed 9 native tests plus JS and WASM regressions. Its production build still
reports the existing Respo DOM FFI and `:unit`/`:nil` warning baseline; neither
external repository was modified.
