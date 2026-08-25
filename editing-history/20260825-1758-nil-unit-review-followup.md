# Nil / Unit review follow-up

- Kept zero-argument `(:: 'Nil)` forms as the builtin Nil annotation in both parser representations.
- Made the JavaScript console formatter render nested Unit values as `&unit` while preserving nested nil handling.
- Added parser, formatter, and typed-EDN Nil decoder regressions for the reviewed edge cases.
- Documented why transaction tests use package metadata: serialized format versions are normalized, so version mutations cannot exercise change detection.
