# Preserve macro FFI classification

- Inspect `SchemaKind` before classifying a nested function annotation as a
  callback boundary.
- Keep macro-kind annotations on the unsupported-type path, with guidance that
  distinguishes compile-time macros from runtime callbacks.
- Add deterministic regression coverage and document the distinction between
  nested macro annotations and macro raw bindings.
