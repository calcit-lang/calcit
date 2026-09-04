# Focus the strict JavaScript nullish fixture

- Correct the nullable-dereference fixture return schema from `JsObject` to
  `Number`, matching the result of `.-length`.
- Keep the type-fail fixture focused on `E_JS_FFI_NULLABLE_DEREF` so unrelated
  return-type validation cannot mask the intended diagnostic.
- This addresses the focused review finding on PR #621.
