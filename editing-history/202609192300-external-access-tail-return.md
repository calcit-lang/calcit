# 修复 ExternalAccess 尾位置丢失 return（JS codegen）

- 背景：`7c80bc2e`（0.17.0 起）把「已静态断言/强转后的 external-object 接收者的裸字段读」
  重写为 `MethodKind::ExternalAccess`，以保留 trait 的 `:names` 映射。但 `emit_js.rs` 中
  `ExternalAccess` 分支没有使用 `return_code`（对照 `ExternalGet` 与 `Access`），
  导致该字段读处于尾位置时只生成裸表达式，随后落入外层 `match`/分支的
  `throw new Error("match: no matching branch for tag ...")` 兜底，运行时误报。
- 影响：Respo 的 `input-event-checked?` 等「`unsafe-coerce` 到 host trait 后读字段」
  的适配器在 0.17.0 下会错误抛错（`(:some ...)` 分支命中却仍抛错）。
- 修复：`MethodKind::ExternalAccess` 生成 `{return_code}{obj}[{prop}]`，与 `ExternalGet` 对齐。
  尾位置得到 `return element["textContent"]`，非尾位置 `return_code` 为空，行为不变。
- 测试：在 `typed_external_field_codegen_uses_ffi_name_overrides` 中新增尾位置断言，
  覆盖 `Some("return ")` 时输出 `return element["textContent"]`。
- 验证：`cargo fmt`、`cargo clippy --release -- -D warnings`、
  `cargo test --release --lib typed_external_field_codegen_uses_ffi_name_overrides`；
  用修复后的 release 二进制对 Respo 重新 `calcit calcit.cirru js` 并跑
  `yarn test-dom-host`，`typed-DOM-host-contract-ok` / `typed-SSR-ref-contract-ok` 通过。
