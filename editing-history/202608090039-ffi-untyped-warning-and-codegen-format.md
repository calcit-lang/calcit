# Untyped FFI access warning and JS codegen readability

- Added opt-in `W_JS_FFI_UNTYPED_ACCESS` diagnostic (enabled with
  `--warn-dyn-method`): raw `.-`/`.!`/`aget`/`aset`/`js-get`/`js-set` on a bare
  `JsObject` receiver with a literal key now suggests declaring an
  external-object trait. Dynamic keys and typed/ nullable receivers are
  skipped; silent by default.
- Improved generated JavaScript readability without a real formatter: function
  bodies and nested statement blocks are indented one level (reusing
  `indent_block`, O(n)), top-level functions are separated by a blank line, and
  a stray-space typo in raised-error codegen was fixed.
- Documented both in `docs/features/js-interop.md` and the repo `Agents.md`.

Validation:

- `cargo test -q`（366 lib + 192 integration）
- `cargo clippy --lib --bin cr -- -D warnings`
- `cargo fmt --check`
- `yarn compile` / `yarn check-all`
- `yarn check-agent-interface`
- `cr docs check-md docs/features/js-interop.md --entry calcit/test.cirru`
- Respo: `cr js` + `yarn test`
