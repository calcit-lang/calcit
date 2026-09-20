# WASM 公开 ABI 统一要求显式 defwasm-export（#1241）

## 背景

历史调试兼容让所有可编译的普通 `defn` 都进入 WASM export surface，导致宿主 ABI 无法区分业务入口与内部工具函数。
0.19.0 起只让显式 `defwasm-export` 与必要 runtime symbols 公开。

## 改动

- `src/codegen/emit_wasm.rs`：第二遍 codegen 中，普通 `defn` 的 `export_name` 置为 `None`，只保留显式
  `defwasm-export` 的导出；依赖编译失败的普通函数仍保留 trapping slot（维持 call/table 索引稳定），但不再
  进入 export section。显式声明无法编译时仍直接失败。
- runtime 保留符号（`memory`、`__heap_ptr`、`__string_tag`、`__str_new`、WASI `_start`、Component 的
  `cabi_*` 与 adapter symbols）由各自的 emitter 管理，不经过用户函数导出路径。
- `calcit/test-wasm.cirru`：把 Node 冒烟会调用的宿主函数批量改为 `defwasm-export`（`edit transaction` +
  `tree replace-leaf`），改用显式声明。
- `scripts/test-wasm-fail-closed.mjs`：改为断言内部 unsupported slots 不再导出，并保留 runtime symbols；
  runtime trap 由 codegen 阶段的显式导出拒绝保证。
- `scripts/wasm-validation.md`：更新 export allowlist 说明与迁移指引。
- `calcit/test-*.cirru` 与 `calcit/test-wasm-suite.cirru`：手动渐进套件入口 `main!` 改为 `defwasm-export`。
- `tests/component_ffi_cli.rs`：共享 fixture 现在包含大量 core-only 显式导出，contract 断言改为按 id 查找
  import/async 定义，而不再假设 `summary.unsupported == 0`。

## 验证

- `yarn try-wasm`：Node 冒烟（175+ 导出）、fail-closed、preprocessing/lowering 失败 fixture 全部通过。
- `calcit wasi` preview：exports 收敛为 `memory, __heap_ptr, __string_tag, __str_new, _start`。
- `cargo test --release` 全绿；`cargo fmt`、`cargo clippy -- -D warnings`、`yarn try-js`、`yarn try-ir`、
  `yarn check-static-method-lowering` 通过。

## 备注

`yarn check-agent-interface` 在当前环境（与本次改动无关）本身失败：它对 `calcit/test.cirru`
`app.main/main!` 使用过期的 `@48.1` 路径；stash 本次改动后同样复现。
