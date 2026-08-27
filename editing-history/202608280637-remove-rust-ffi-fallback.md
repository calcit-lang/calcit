# Remove Rust FFI fallback / 删除 Rust FFI fallback

## 中文

- sync、callback、blocking 调用缺少 C-safe version/method/free symbol 时直接返回包含期望符号的迁移错误，不再查找或调用 Rust `Vec<Edn>`/trait-object function pointer。
- 删除 `EdnFfi`、`EdnFfiFn`、release-host legacy warning、build identity validation，以及 `abi_version`/`edn_version` probes。
- 删除 `--ffi-build-id` 与 build.rs 中 rustc/target/debug/panic identity 生成，C-safe wire protocol 不再绑定 host 与 dylib 的 Rust toolchain。
- native capability verifier 改为验证 buffer/async/resource protocol versions 和配套 free/release symbols；realization key/receipt 记录 protocol versions，并继续校验 artifact size/hash。
- 更新 FFI bindings、async protocol、upgrade guide 与 quick reference，明确 C-safe v1 是唯一支持边界。
- 明确 version、method、free 和 release 符号按模块实际能力要求；只实现异步 callback 的模块不导出 buffer protocol。
- 真实验证：当前 debug Calcit host 加载 release calcit-paint dylib 的 macOS blocking GUI smoke 通过，paint 也通过 verifier；只导出 legacy symbol 的 C fixture 在 caps 和 sync/async/blocking runtime 都确定性拒绝，且不会进入 Rust symbol。

## English

- Sync, callback, and blocking calls now return migration errors naming the expected C-safe version/method/free symbols; they no longer look up or invoke Rust `Vec<Edn>`/trait-object function pointers.
- Removed `EdnFfi`, `EdnFfiFn`, release-host legacy warnings, build-identity validation, and `abi_version`/`edn_version` probes.
- Removed `--ffi-build-id` and build.rs rustc/target/debug/panic identity generation; the C-safe wire protocol no longer couples host and dylib Rust toolchains.
- Native capability verification now checks buffer/async/resource protocol versions and matching free/release symbols. Realization keys/receipts record protocol versions while retaining artifact size/hash integrity checks.
- Updated FFI bindings, async protocol, upgrade guide, and quick reference to make C-safe v1 the only supported boundary.
- Clarified that required version, method, free, and release symbols are capability-specific; async-only modules do not export the buffer protocol.
- Real validation: the current debug Calcit host passes a macOS blocking GUI smoke against the release calcit-paint dylib, which also passes verification; a C fixture exporting only a legacy symbol is deterministically rejected by caps and sync/async/blocking runtime paths without entering that Rust symbol.
