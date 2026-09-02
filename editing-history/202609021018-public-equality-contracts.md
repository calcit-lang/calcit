# Public equality contracts / 公共相等性同类型契约

## Summary / 概要

- Changed public `=`, `not=`, and `/=` schemas from `Dynamic` operands to a shared generic `T`, including variadic rest arguments for `=`.
- Added definition-attached tests, examples, upgrade guidance, and a focused negative test for mixed-type diagnostics.
- Added equality-specific migration guidance to `W_FN_ARG_TYPE_MISMATCH` without changing the raw runtime primitives `&=` and `&compare`.
- Kept `assert=` as an intentional open macro boundary. Its generated runtime assertion explicitly coerces captured expressions locally before calling raw `&=`, so assertions can still inspect arbitrary runtime values without weakening public equality.
- Migrated the nominal cross-type struct test to explicit raw equality and narrowed the JavaScript `typeof` result to `String` at the FFI boundary with `assert-type`.

## Quality movement / 质量变化

- `schemaDynamic`: 297 -> 291
- `unresolved`: 203 -> 197
- `typeNotFull`: 144 -> 141
- `codeDynamic`, `codeNil`, `declaredOptional`, `unsafeCoerce`: remain 0

## Ecosystem regression / 生态回归

- Respo check-only reaches one expected migration point in `respo.css/detect-nodejs?`: a raw JS value must be narrowed before comparing it with `|node`.
- Recollect analysis is currently blocked earlier by its installed Respo dependency's pre-existing legacy Dynamic macro schema; this change did not introduce that blocker.

## Validation / 验证

- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface` (18/18)
- `yarn check-all`
- Calcit core unit tests (233/233), equality examples, upgrade markdown examples, quality baseline, native/JS/WASM integration checks
