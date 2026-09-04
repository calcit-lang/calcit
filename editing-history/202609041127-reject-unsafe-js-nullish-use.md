# Reject unsafe JavaScript nullish use in strict source

- Promote direct member dereference of `JsNullish<JsObject>` to
  `E_JS_FFI_NULLABLE_DEREF` for strict project source; optional access and
  explicit `js-present?`/`js-nullish?` narrowing remain supported.
- Promote legacy `nil?`/`some?` checks on `JsNullish<T>` to
  `E_JS_FFI_NULLABLE_PREDICATE`, preserving the distinction between host
  `null`/`undefined` and Calcit absence.
- Keep both established warnings in compatibility mode and outside the strict
  project-source scope.
- Add unit and type-fail coverage for both stable error codes and document the
  migration paths.
- Revalidate `calcit-lang/js-ffi` Node and browser entries against the combined
  strict stack with no new nullable-boundary failures or quality debt.
