# Calx lowering review hardening / Calx lowering 审阅加固

## 中文

- 处理 PR #532 review：生成阶段不再用 map 下标直接访问 `LocalId`。typed local 若没有对应的 emitted Calx local，会返回带 definition 与 source tree path 的 `CalxKernelCompileError::Lowering`，不再存在异常 snapshot 触发 panic 的路径。
- 由于 `BodyBuilder` 的 structured-control closure 只接受 `CalxBuildError`，emission context 记录首个 lowering invariant failure，并在 function 加入 `ProgramBuilder` 之前返回；不会生成或验证部分 program。
- 增加异常 lowering plan 的回归测试，固定 missing local index 的结构化错误行为。
- 从 `calcit::codegen::calx` re-export 公共错误枚举 payload 使用的 `CalxBuildError`、`CalxProgramError` 与 `CalxError`，下游无需声明匹配版本的直接依赖即可匹配错误。
- 验证通过：`cargo fmt`、590 个 library tests 与全部 binary/doc tests、`cargo clippy --all-targets --all-features -- -D warnings`、`yarn compile`、`yarn check-all`、`yarn check-agent-interface`（17/17）。

## English

- Address PR #532 review feedback by removing direct map indexing for `LocalId` during emission. If a typed local has no emitted Calx local, lowering now returns `CalxKernelCompileError::Lowering` with its definition and source tree path instead of allowing a malformed snapshot to panic.
- Because `BodyBuilder` structured-control closures accept only `CalxBuildError`, the emission context records the first lowering invariant failure and returns it before adding the function to `ProgramBuilder`; no partial program is built or validated.
- Add a regression test for the structured missing-local error from a malformed lowering plan.
- Re-export `CalxBuildError`, `CalxProgramError`, and `CalxError` from `calcit::codegen::calx`, matching the payload types exposed by the public error enums so downstream users do not need a version-matched direct dependency merely to pattern-match errors.
- Verification passes `cargo fmt`, 590 library tests plus all binary/doc tests, `cargo clippy --all-targets --all-features -- -D warnings`, `yarn compile`, `yarn check-all`, and `yarn check-agent-interface` (17/17).
