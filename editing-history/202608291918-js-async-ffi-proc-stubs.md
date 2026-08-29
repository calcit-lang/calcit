# Link native async FFI procedures in JavaScript output

## Context

The shared core Snapshot contains typed `FfiResponse` and `FfiTask` adapters. JS
codegen therefore imports their underlying registered procedure names even in
browser programs that never call the native-only APIs. The npm runtime omitted
those exports, so bundlers diagnosed statically undefined imports.

## Change

- Export the response resolve/reject and task cancel names through the existing
  JavaScript `unavailableProc` boundary.
- Assert that all generated async FFI procedure imports are linkable from the
  compiled npm runtime.

Native behavior remains unchanged. The stubs only make the shared JS module
graph complete and retain the established unsupported-procedure behavior if a
native-only API is accidentally reached on JavaScript.
