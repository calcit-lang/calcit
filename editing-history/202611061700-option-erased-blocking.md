# Option 擦除回 nil 改为错误 `E_OPTION_ERASED_TO_NIL`（#2124）

0.36.0 把 `W_OPTION_ERASED_TO_NIL` Notes 升级为阻断错误，与 `E_BROAD_TYPE` 走同一条路径。2026-11-06 生态普查共 189 处（edn-renderer 176），库迁移并行进行；依赖库里的写法本来就不报，所以下游不必等上游。

- `snapshot::OPTION_ERASED_ERROR`；preprocess 的三个报告点改用它，消息改为 `[Error]`，免报规则（宿主实参、`JsNullish` 形参与返回）不变。
- `preprocess::is_blocking_advisory` 覆盖两种错误码，`split_blocking_advisories` 据此拆分，所以 `--check-only`、`--all-defs` / `--ns`、`--incremental` 与运行/JS 预检都会拒绝。
- `cr.rs` 的 `report_broad_type_errors` 按错误码分组打印，汇总行带 `[E_OPTION_ERASED_TO_NIL]`，JS 的 `calcit.build-errors.mjs` 里也能看到错误码。
- 测试：`type_fail.rs` 改用新错误码；`strict_check_cli` 的 all-defs 用例改为断言 definition failed；`option-nil-roundtrip` fixture 故意保留擦除，view 测试改为断言 strict check 只因该错误失败，它的值语义测试只留 `deprecated_aliases_cli` 里的 native `calcit test`，package.json 的 run-core-tests 只回放不擦除的 `nil-macro-match`（擦除定义逐 target 编译会被预检拒绝），view 测试另断言恰好那 8 个擦除定义失败；`check-known-assertion.mjs` 的 binding-proof 回放只回放不擦除的定义，另断言五个 `(:none) nil` 定义被拒绝，JS 模式接受预检先报的擦除错误。
- 文档：`static-analysis.md`、`cli-options.md`、`upgrade.md` 0.36.0 小节。
