# Cover optional untyped JavaScript access

- Extend strict `E_UNTYPED_JS_OBJECT_ACCESS` enforcement and compatibility
  inventory to optional property access (`.?-`) and optional native invocation
  (`.?!`) on bare `JsObject` receivers.
- Exercise all four native member operation kinds in the strict regression test
  and document the complete audited syntax family.
- This closes the optional-operation bypass identified during review of PR #616.
