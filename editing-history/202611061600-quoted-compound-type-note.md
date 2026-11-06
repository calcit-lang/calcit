# 带 quote 的复合类型报 `W_QUOTED_COMPOUND_TYPE`（#2583 的 0.36.x 部分）

#2581/#2582 之后，只放类型的位置都接受裸写的复合类型，`edit format` 也会把 `quote $ List Number` 改成 `List Number`。0.36.x 先给提示，0.37.0 再在加载时拒绝。

- `snapshot::collect_quoted_compound_type_hits`：对项目定义的代码和 examples 跑 `unquote_compound_types`，范围与 `edit format` 完全一致，所以格式化后提示必然消失。`:tests` 不扫描（`edit format` 不改测试）。
- `cr.rs` 的 `push_quoted_compound_type_advisory` 汇总成一条 Notes（总数、定义数、第一处），避免大库刷屏；放进 `push_snapshot_level_advisories`，普通和 `--incremental` 检查都会报。
- `docs/run/upgrade.md` 加 0.36.x 小节。

验证：`tests/strict_check_cli.rs::quoted_compound_types_are_a_note_until_edit_format`（普通与 incremental 都报，`edit format` 后消失）。
