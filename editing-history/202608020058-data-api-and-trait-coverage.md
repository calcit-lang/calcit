# Data API and trait coverage

## Review outcome

- Corrected the core `Option` and `Result` declarations to carry their real generic parameters instead of dynamic payloads.
- Added the missing high-frequency predicate, fallback, chaining, and error-mapping helpers, with schemas, method bags, docs, and queryable examples.
- Added the documented-but-missing `Compare` trait for Number and String.
- Connected the existing `Countable` and `Contains` traits to List, Map, Set, String, Record, and Tuple/enum values for method dispatch, static `:where` checks, and runtime `assert-traits`.

## Cross-platform consistency

- Made Number and String `.compare` visible through the shared built-in method bags so native, JavaScript, and WASM preprocessing agree.
- Aligned Rust and JavaScript trait introspection for Record/Struct and Tuple/Enum by merging their built-in and attached impl records.
- Added native, JavaScript, and WASM regression coverage for the new APIs and trait capabilities.

## Type-system fix

- Named generic enum values now safely satisfy builtin dynamic-tuple parameters, while a dynamic tuple still cannot satisfy a concrete named enum.
- Type-definition resolution now unwraps `def` and `impl-traits` and resolves unqualified core enum references.
- Tracked the underlying generic-enum diagnostic defect in <https://github.com/calcit-lang/calcit/issues/287>.

## Documentation

- Expanded the data-type overview to distinguish persistent values, named data, executable values, and explicitly stateful containers.
- Documented Cirru EDN versus JSON fidelity, unsupported values, and restoring declared Record/Enum identity during parsing.
- Added the built-in trait matrix and the Option/Result helper surface to the polymorphism guide.
