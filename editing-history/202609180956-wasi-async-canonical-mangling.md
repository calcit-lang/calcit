# Align WASI async canonical mangling / 对齐 WASI async canonical mangling

## 中文

- 将 async Component core adapter 从 Calcit 私有 import 名称迁移到 `wit-component` 可识别的 legacy Canonical ABI mangling。
- stackful async export 使用 `[async-lift-stackful]<symbol>`，async import 使用 `[async-lower]<symbol>`。
- `task.return` 使用 `[export]$root/[task-return]<symbol>`；waitable 与 subtask builtins 使用 `$root` 下的标准括号名称。
- 同时保留 `$root` 与 `[export]$root` 为编译器内部模块，用户声明相同模块时继续 fail closed。
- 更新真实 Calcit fixture 的 Node lifecycle smoke 与中文文档；同步 Component export 仍保持原名称和 ABI。
- 使用 calcit-bindgen 的 async WIT 分支执行跨仓库 smoke，真实 Calcit IR v3 与 core module 已通过 `embed_component_metadata` 和 `ComponentEncoder` 打包。

## English

- Migrate the async Component core adapter from private Calcit import names to the legacy Canonical ABI mangling recognized by `wit-component`.
- Use `[async-lift-stackful]<symbol>` for stackful async exports and `[async-lower]<symbol>` for async imports.
- Use `[export]$root/[task-return]<symbol>` for `task.return`, plus the standard bracketed waitable and subtask builtin names under `$root`.
- Reserve both `$root` and `[export]$root` as compiler-internal modules and continue to fail closed when user declarations collide with either module.
- Update real Calcit fixture Node lifecycle smokes and Chinese documentation while preserving synchronous Component export names and ABI.
- Run a cross-repository smoke with the calcit-bindgen async WIT branch; real Calcit IR v3 and its core module package successfully through `embed_component_metadata` and `ComponentEncoder`.

## Verification / 验证

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo test codegen::emit_wasm`
- `cargo test --test component_wasm_cli`
- cross-repository `calcit-bindgen` async Component packaging smoke
