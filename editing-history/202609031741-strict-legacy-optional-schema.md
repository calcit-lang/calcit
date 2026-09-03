# Reject legacy Optional schemas in strict mode

- Promote public `Optional<T>` schemas from a migration warning to the strict-mode `E_LEGACY_OPTIONAL_SCHEMA` error.
- Keep compatibility mode warning-only, and preserve raw core `&...` primitives plus `optionally` as explicit internal nullable bridges.
- Require public APIs to choose `Option`, `Result`, `Unit`, or `JsNullish` according to the actual absence, failure, effect, or JavaScript-boundary semantics.
