# Receiver-first unwrap examples

- Migrated the built-in `option:unwrap-or` and `result:unwrap-or` API examples
  to the typed receiver-first method form.
- Verified the examples through `analyze check-examples`, the Markdown snippet
  through `docs check-md`, and generated JavaScript by executing the trait test
  bundle with Node.js.
- Kept the two WASM fixtures in function form: the internal WASM backend does
  not yet lower enum constructor receiver-method calls, while its complete
  verification suite continues to pass with the supported form.
