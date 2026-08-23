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
- Regression coverage includes the shared-binding matcher and literal thunk
  inference. The CLI reproduction now fails during preprocessing when a
  `String` none branch is combined with a numeric Option payload and `identity`.
