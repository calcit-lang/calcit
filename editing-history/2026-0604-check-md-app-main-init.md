# 2026-06-04 check-md 入口加载与 app.main 注入

- `prepare_program_for_snippet` 加载 entry snapshot 失败时不再回退空 snapshot，改为直接报错。
- `extract_modules_from_edn` 对畸形 `configs.modules` 返回明确 EDN 预览，不再静默忽略。
- check-md 注入片段固定使用 `app.main/main!`，避免误跑 entry 项目的 `:init-fn`（如 memof 测试套件）。
- `docs/run/cli-options.md` 注明 Run 模式使用 `app.main/main!`。
