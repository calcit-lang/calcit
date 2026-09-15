# Component Float32 NaN boundary

- Preserve the source-level `Float32` refinement invariant at Component boundaries: incoming canonical f32 NaN values trap before entering Calcit.
- Apply the same check to direct scalar adapters and recursively lifted canonical memory; finite values and infinities retain the existing exact-promotion behavior.
- Verification: `cargo test --test component_wasm_cli`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`.
