# Type macro-generated functions without public schemas

- Strict public-schema enforcement must distinguish source definitions from definitions emitted while a macro frame is active. Generated closures have no independently maintainable `CodeEntry.schema`, so their callable contract is synthesized from the expected callback type and the inferred body return instead.
- Generic callback arguments are preprocessed left to right while retaining bindings proven by earlier arguments. This lets generated callbacks receive concrete parameter types, and `option:fold` preserves its input/output relation through generated `if-let` closures.
- Macro-generated functions inherit the enclosing lexical feature set. In particular, a generated closure inside a reviewed `:js-ffi` adapter must not lose that capability boundary.
- CLI definition-test wrappers are compiler-owned zero-argument functions and receive a structured `Fn` schema when inserted into the temporary Snapshot.
- Keep `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` unchanged for ordinary source definitions; the exemption is based on macro provenance, not synthetic naming conventions.

Verification: `RUST_MIN_STACK=16777216 cargo test`, `cargo clippy --all-targets -- -D warnings`, `yarn check-all`, and strict/compatibility consumer checks against Respo main. Respo compatibility mode also passes 40/40 definition tests, JS compilation, and a browser load through Vite without console errors; strict mode now advances past generated-schema validation to the project's genuine `Option<dynamic>` migration debt.
