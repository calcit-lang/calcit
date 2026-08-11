## Trailing Option parameters

- Treat only the continuous `Option<T>` suffix in a typed fixed-arity function schema as omittable; an earlier `Option<T>` does not make later required parameters optional, and rest parameters keep their existing behavior.
- Fill omitted arguments with the nominal `calcit.core/Option` `%none` value in the native runtime and with the matching `%none` enum value in generated JavaScript.
- Keep preprocessing arity checks and method-call validation aligned with runtime behavior so accepted calls do not produce false arity diagnostics.
- Consolidate the end-to-end coverage beside the legacy `?` argument tests, documenting `Option` as the preferred replacement for `?` and `nil` in new typed APIs.
