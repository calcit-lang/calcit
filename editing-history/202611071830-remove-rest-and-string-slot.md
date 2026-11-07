# 删除裸 `& T` rest 写法和字符串 type slot 绑定（0.37.0）

0.36.x 的 `W_FN_REST_SPELLING` 和 `W_TYPE_SLOT_STRING` Notes 改为加载错误 `E_LEGACY_TYPE_SYNTAX`。

- `snapshot::check_removed_0_37_spellings` 在 `load_snapshot_data_inner` 开头扫描原始 EDN：`:schema` 里 `Fn` / `Macro` 的裸 `&`（复用 `collect_fn_rest_spelling_hits`），以及每个 entry 的 `:type-slots`（复用 `collect_type_slot_spelling_hits`）。依赖库也算，和 #2583 一致。`edit format` 走 `load_snapshot_data_for_format`，仍能读取旧写法并迁移。
- 函数体里 `hint-fn` 等代码类型位置的裸 `&` 不在范围内，`edit format` 也不改写它们。
- `cr.rs`：删掉 `RawSpellingHits` 和两条 Notes；incremental 路径的 Snapshot 可能来自缓存，改为调用 `check_raw_removed_spellings` 重读文件。
- 测试：`strict_check_cli` 的 incremental 用例改为断言冷、热两次都报错，`edit format` 能迁移；新增 `string_type_slot_bindings_are_rejected_until_edit_format`。三个 fixture 改成 `(& T)`。
- 文档：`upgrade.md` 新增 0.37.0 小节，`cli-options.md`、`project-structure.md` 同步。
