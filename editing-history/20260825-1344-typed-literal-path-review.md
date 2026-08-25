# Typed literal path review follow-up

- Suppressed diagnostics only while reprocessing compiler-generated path
  guards; original caller expressions are checked before specialization.
- Added return-type inference for preprocessed `match` forms so expanded
  `get-in` retains its `Option<T>` result type.
- Allowed a Dynamic final `get-in` payload while retaining the stricter static
  final-payload requirement for Map-only `assoc-in`.
- Expanded the runtime fixture to cover nil intermediates, stored Options,
  indexed lookup, runtime Struct rejection, fallback behavior, and actual
  path/replacement side-effect order on native and generated JS.
- Made `yarn bench-literal-paths` build the JS runtime, execute correctness
  assertions, verify typed/dynamic structural equality, and report warm native
  and generated-JS median samples.

Validation: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D
warnings`, `cargo test`, `yarn check-all`, and `yarn bench-literal-paths`.
