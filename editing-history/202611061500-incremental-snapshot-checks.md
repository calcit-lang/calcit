# `--check-only --incremental` 补上 Snapshot 级诊断

`--check-only --incremental` 走单独的加载路径（`analysis_cache::load_snapshot_for_incremental_analysis`），之前没有调用 `push_broad_type_advisories`、type slot / rest 写法扫描和 `W_OPEN_FAMILY` 汇总。结果是带裸 `Fn` 的项目在 `--incremental` 下显示 `✓ Check passed`，而普通 `--check-only` 报 `E_BROAD_TYPE` 失败。

- `cr.rs` 新增 `push_snapshot_level_advisories`，普通加载路径和 incremental 路径共用。incremental 路径的 Snapshot 来自缓存，看不到原始 EDN，所以 `raw_spelling_hits` 重新读一次 Snapshot 文件做 `W_TYPE_SLOT_STRING` / `W_FN_REST_SPELLING` 扫描（只解析 EDN，开销很小）。
- 缓存命中（warm）时不编译，但仍取出这些诊断：`E_BROAD_TYPE` 让检查失败，其余作为 Notes 打印。闭包外的定义变化不会让缓存失效，所以这一步不能省。
- 通过后打印 `W_OPEN_FAMILY` 汇总，与普通 `--check-only` 一致。

验证：`tests/strict_check_cli.rs` 新增 `incremental_check_reports_snapshot_level_diagnostics`，覆盖 cold/warm 两次都输出 `W_FN_REST_SPELLING`，以及闭包外改成裸 `Fn` 后 warm 缓存下仍报 `E_BROAD_TYPE` 失败。
