# check-md 与 EDN 报错（2026-06-03）

- `collect_check_md_module_paths`：用 `extract_modules_from_edn` 只读 `configs.modules`，避免对旧格式大快照（如 `demos/calcit.cirru` 的 `%Expr` 代码字段）做全量 `load_snapshot_data`。
- `check-md`：`prepare_program_for_snippet` 以 entry 快照为底、最后写入 `app.main`；no-run 优先编译片段里的 `app.main` defs；片段若全是顶层 `def/defn/defcomp/...` 则拆成多个 `CodeEntry`（`create_file_from_snippet`）。
- `data/edn.rs`：反序列化失败时在 `got:` 里把旧 `%Expr`/`%Leaf` 还原为格式化 Cirru（约 520 字上限），`snapshot`/`call_stack` 错误预览走同一套。
