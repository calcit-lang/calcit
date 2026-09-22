# 按语义职责拆分 preprocess 的 JS FFI 校验（#1243 有界切片）

## 背景

#1243 要求只在语义边界稳定后做一个有界、不改变用户语义的拆分。选择 `src/runner/preprocess/mod.rs`
中近期持续改动的 JS FFI 能力与边界校验作为 owner：它与刚收敛的 FFI/类型边界工作（#1241、#1273、defexternal）
直接相邻，职责单一（`:js-ffi` 能力门禁、host target 校验、external-object trait 字段检查、nullable/untyped 访问诊断）。

## 改动

- 新增 `src/runner/preprocess/js_ffi.rs`，迁入 17 个函数（约 530 行）：
  `require_js_ffi_feature`、`validate_js_ffi_target`、`validate_js_ffi_definition_target`、
  `js_ffi_operation_name`、`require_js_ffi_feature_for_operation`、`ffi_metadata_target/value`、
  `reject_or_warn_on_nullable_js_ffi_dereference`、`static_js_field_name`、
  `rewrite_typed_js_field_operation`、`check_typed_js_field_operation`、
  `reject_or_warn_on_untyped_js_ffi_field_access`、`reject_or_warn_on_legacy_js_nullish_predicate`、
  `is_external_trait_field`、`trait_is_external_object`、`external_trait_field_is_writable`、
  `current_function_has_js_ffi_feature`。
- `mod.rs` 只保留 `mod js_ffi; use js_ffi::*;` 与 `pub(crate) use js_ffi::trait_is_external_object;`
  （后者供 `type_inference.rs` 继续使用），减少约 530 行。
- 子模块通过 `use super::*` 读取父模块的私有状态（`CURRENT_FN_FEATURES` 等），未新增跨层 public API，
  也未改变函数签名、诊断 code、顺序或输出。

## 验证

- `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test --release` 全绿。
- `yarn try-rs`、`yarn try-js`、`yarn try-wasm`、`yarn check-strict-default`、`yarn check-js-runtime` 通过。
- 纯代码移动：`git diff` 仅删除原函数定义并新增模块声明；无逻辑改动。

## 备注

`cargo clippy --all-targets -- -D warnings`（即 `yarn lint-rs`）在 `src/codegen/emit_wasm/edn_parse.rs`
因既有 `items_after_test_module` 失败；stash 本次改动后同样复现，属仓库既有问题，CI 使用
`cargo clippy -- -D warnings` 不受影响。
