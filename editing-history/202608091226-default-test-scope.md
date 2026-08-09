# Keep default test discovery project-local

- Documented that unscoped `cr test` discovers only namespaces defined by the
  input snapshot, excluding tests from embedded core and loaded modules.
- Added a regression with a passing project test and an intentionally failing
  `calcit.core` test. The default command must pass without selecting core;
  explicitly targeting that core definition must still execute and fail.
- Kept the embedded `calcit.test` coverage in `calcit-core.cirru`; maintainers
  can run it explicitly with `cr test calcit.test`.

Validation:

- `cargo fmt`
- `cargo test --bin cr default_test_scope_excludes_core_and_dependency_namespaces`
- `cargo clippy -- -D warnings`
- `cargo test` (372 library, 2 caps, 197 cr tests passed)
- `yarn compile`
- `yarn check-agent-interface` (12/12)
- `yarn check-all` (native, JavaScript, and WASM passed)
- Respo default JSON test listing selected 0 project tests and no embedded core
  tests; stdout remained one parseable line
- `git diff --check`
