# check-md 多 def 片段与 no-run 增强

## 问题

1. `create_file_from_snippet` 把 markdown 片段整段包进 `app.main/main!`，多行 `def`/`defcomp` 无法分别做预处理检查。
2. `check-md` 用 entry 的 `init-fn` 编译成功时，不会检查片段里的 `app.main` 定义。
3. `check-md` 只加载 `configs.modules`，不加载 entry `calcit.cirru` 自身命名空间（如 `respo.core`），导致带 `respo.core/...` 的 `no-run` 块失败。
4. 依赖模块合并时可能覆盖注入的 `app.main` 片段。

## 改动

- `snapshot.rs`：`create_file_from_snippet` 在片段均为顶层 `def/defn/defcomp/...` 时，为每个定义单独建 `CodeEntry`。
- `docs.rs`：`prepare_program_for_snippet` 以 entry 快照为底、最后写入 `app.main`；`run_check_only_in_process` 优先编译 `app.main` 内定义；失败时输出完整错误行。
- respo 文档：README / beginner-guide / component-states / dom-events / styles / Respo-Agent 中部分 `cirru.no-check` 升为 `cirru.no-run`（补 `ns` 与 `:require`）。

## 验证

```bash
cd respo/respo
cr calcit.cirru docs check-md -d calcit.cirru README.md
cr calcit.cirru docs check-md -d calcit.cirru docs/beginner-guide.md
```

## EDN 报错简化（同次）

- `data/edn.rs` 新增 `compact_edn_for_format`、`format_edn_display`、`format_deserialize_error`。
- 旧快照 `%Expr` / `%Leaf` / `CodeEntry.code` 在 `got:` 里通过 `legacy_snapshot_edn_to_cirru` 还原为格式化 Cirru 片段（上限约 520 字符），便于调试；不再 dump 整棵 `EdnRecordView` Debug 树。
- `snapshot.rs` 的 `from_edn` 失败与 `got:` 预览统一走紧凑格式；`.calcit-error.cirru` 栈信息同样压缩。
