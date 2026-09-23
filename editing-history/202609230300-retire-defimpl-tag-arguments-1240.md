# defimpl tag 参数退役

- #1240 的一部分：`calcit.core/defimpl` 现在要求 impl 名与 trait 都是 symbol；旧的 tag 参数（originless inherent method bag）以 `E_LEGACY_DEFIMPL_TAG` 拒绝，并给出 `(defimpl ImplName Trait ...)` 迁移提示。core 内已无 tag 形式使用；生态验证显示 memof 在 0.0.28+ 已迁移为 nominal `deftrait`，其余远端消费者不再使用 tag-trait `defimpl`。
- 宏内 `tag?` 接受分支、`turn-tag` trait 转换与扁平 method bag（未加 `.method` 键）分支一并移除；宏只接受 `(.method value)` 成对列表，非法键仍以原提示拒绝。
- `calcit edit format` 的 `W_LEGACY_INHERENT_IMPL` 迁移告警与 `count_legacy_inherent_impls` 已删除，因为用户层不再能通过 `defimpl` 产生这种 impl；core bootstrap 直接调用 `&impl::new` 的 originless bag、strict 分派诊断与对应 Rust 测试保留，作为内部动态边界的防御。
- 负向编译诊断通过预处理器测试 `defimpl_rejects_legacy_tag_arguments` 覆盖（宏展开期 raise 无法由 definition `:tests` 表达）；nominal 正常路径由 `calcit/test-traits.cirru` 与 core definition tests 验证。
- 文档 `docs/features/traits.md`、`docs/features/polymorphism.md`、`docs/run/upgrade.md` 已更新；`docs check-md` 70 文件 / 339 blocks 通过。`cargo fmt`、`cargo clippy -- -D warnings`、`cargo test --release`、`yarn try-core-tests`、`yarn check-strict-default`、`yarn check-core-dynamic-classification`、`yarn try-rs`、`yarn try-js`、`yarn try-wasm` 全部通过。
