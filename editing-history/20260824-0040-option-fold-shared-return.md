# option:fold shared callback return type

- Higher-order callback matching must reuse the enclosing generic binding map.
  A fresh map per callback makes repeated variables such as `option:fold`'s
  return `U` independent and silently accepts incompatible branch results.
- Unhinted zero-argument thunks may safely retain a concrete return type from
  their final expression. This supplies the none-branch binding for `U` while
  keeping unhinted callbacks with parameters dynamic, avoiding retroactive
  tightening of existing higher-order call sites.
- The inferred thunk annotation must preserve `SchemaKind::Macro` for
  `defmacro` forms, matching the explicit-hint path; a dedicated regression
  test prevents zero-argument macros from being exposed as ordinary functions.
- Callback signature matching stages generic bindings and commits them only on
  success, so a rejected nested callback cannot constrain a later argument.
  `option:fold` additionally recovers a concrete return type for unhinted
  callback bodies during its own argument check, including parameterized
  callbacks, without globally tightening unrelated higher-order calls.
- Preprocessing regressions cover direct folds, parameterized callbacks, and a
  user macro expanding to `option:fold`; empty unhinted functions remain
  dynamic rather than treating their parameter list as a return value.
- Regression coverage includes the shared-binding matcher and literal thunk
  inference. The CLI reproduction now fails during preprocessing when a
  `String` none branch is combined with a numeric Option payload and `identity`.
