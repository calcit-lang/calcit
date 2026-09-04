# Preserve opt-in JavaScript object inventory outside project source

- Keep `W_JS_FFI_UNTYPED_ACCESS` behind `--warn-dyn-method` when strict
  preprocessing visits dependency namespaces outside the project-source lint
  scope.
- Retain `E_UNTYPED_JS_OBJECT_ACCESS` for strict project source and add a
  serialized regression test that configures project namespaces explicitly.
- This addresses the focused review finding on PR #620 without broadening the
  hard-error boundary or changing dynamic-key semantics.
