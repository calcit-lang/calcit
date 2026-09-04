# Reject untyped JavaScript object access in strict source

- Promote literal member reads, calls, and writes on a bare `JsObject` from the
  opt-in `W_JS_FFI_UNTYPED_ACCESS` inventory warning to the stable strict error
  `E_UNTYPED_JS_OBJECT_ACCESS` for project source.
- Keep compatibility-mode inventory warnings and dynamic-key raw lookup
  semantics, while typed external-object and nullable receivers continue
  through their dedicated checks.
- Add unit and type-fail coverage plus migration guidance toward minimal
  external-object traits inside lexical `:js-ffi` adapters.
- Validate the migration against `calcit-lang/js-ffi`: `process.argv` now uses a
  `ProcessArgvHost` capability and retains runtime `length` validation without
  increasing its existing explicit-unsafe quality baseline.
