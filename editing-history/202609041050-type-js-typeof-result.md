# Type the JavaScript `typeof` result

- Infer `js/typeof` as concrete `String` before and after raw JavaScript
  lowering; arbitrary `js/...` operations remain opaque
  `JsNullish<JsObject>` boundaries.
- Added focused inference coverage for source and preprocessed forms and
  documented the single language-defined exception in the JS interop guide.
- Revalidated the unchanged `calcit-lang/js-ffi` default, Node, and browser
  entries with the strict raw-primitive and lexical `unsafe-coerce` stack.
  Its existing quality baseline remains at zero Dynamic/unresolved debt and
  30 reviewed coercions, while all Node/browser contract tests and the Vite
  production build pass.
