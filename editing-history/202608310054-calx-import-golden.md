# Calx typed imports 与 correctness golden

## 中文

本次修改完成 Calcit→Calx correctness prototype 的剩余闭环：显式 typed host imports、固定 generated-program golden 和 runtime trap golden。

- 新增 `CalxHostImport` / `CalxHostImports`。embedding 必须以 Calcit definition 为 capability key 显式提供 binding；未知调用不会自动成为 import。
- eligibility 同时校验 typed snapshot 中的 fixed-arity Number/Bool/Unit 签名与 host declaration。签名不一致返回 `CALX_SUBSET_HOST_CAPABILITY`，并在 lowering 前整体 fallback。
- lowering 只为实际可达的 capability 生成 `ProgramBuilder::import_at` 与 `call_import`，并把对应 binding 固定在 compiled kernel 中。未使用的配置不会进入 program。
- zero-result callback 使用 `Result<(), CalxError>`，single-result callback 使用 `Result<CalxValue, CalxError>`。每次运行创建独立 VM；host callback error 保持为 runtime trap，不自动重跑 Calcit。
- `stable_program_summary()` 固定 import declaration、guest syntax 与 Calcit tree origin，供 repository golden 使用，不作为序列化 ABI。
- 新 fixture 覆盖 void observe、value scale、host trap、native/Calx differential result，以及 import signature mismatch fallback。

验证要求包括 focused Calx tests、全部 Rust tests、strict Clippy、`yarn compile`、`yarn check-all` 和 Agent interface suite。

## English

This change completes the remaining correctness loop for the Calcit-to-Calx prototype: explicit typed host imports, a fixed generated-program golden, and a runtime-trap golden.

- Added `CalxHostImport` / `CalxHostImports`. The embedding must explicitly bind a capability using a Calcit definition as the key; unknown calls never become imports automatically.
- Eligibility verifies that the fixed-arity Number/Bool/Unit signature in the typed snapshot exactly matches the host declaration. A mismatch produces `CALX_SUBSET_HOST_CAPABILITY` and falls back for the whole kernel before lowering.
- Lowering emits `ProgramBuilder::import_at` and `call_import` only for reachable capabilities, then retains exactly those bindings in the compiled kernel. Unused configuration does not enter the program.
- Zero-result callbacks use `Result<(), CalxError>` and single-result callbacks use `Result<CalxValue, CalxError>`. Every run creates an independent VM; a host callback error remains a runtime trap and never reruns Calcit automatically.
- `stable_program_summary()` fixes import declarations, guest syntax, and Calcit tree origins for repository goldens; it is not a serialized ABI.
- The new fixture covers a void observer, a value-returning scale operation, a host trap, native/Calx differential results, and fallback for an import signature mismatch.

Required verification includes focused Calx tests, the complete Rust suite, strict Clippy, `yarn compile`, `yarn check-all`, and the Agent interface suite.
