# 并行拆分 CI 并加速 docs check-md

- `Test` job 原先串行执行 cargo test(139s)、docs(72s)、CLI 检查、clippy，整体约 314s。
- 拆成 `rust_tests` / `core_checks` / `docs_checks` / `clippy` 四个并行 job，并保留同名聚合 job `Test`
  （`needs` 四个分片，全绿才通过），避免破坏 branch protection 的 required check 名称。
- `scripts/check-docs-md.sh` 原先对 69 个 md 文件逐个启动 `calcit docs check-md`，每个进程都要重新
  加载 core snapshot；改为有界并发（`CALCIT_DOCS_CHECK_JOBS`，默认 4），结果按文件名排序后统一汇总。
  本地顺序 45.9s → 并发 23.1s；CI 预计 72s → ~25s。汇总行格式与原先一致。
- 兼容约束：脚本仍优先使用 `./target/debug/calcit`，缺失时回退 `cargo run`；`CI=true` 时仍传 `--quiet`。
- 验证：`bash -n scripts/check-docs-md.sh`、`CI=true CALCIT_DOCS_CHECK_JOBS=4 bash scripts/check-docs-md.sh`
  输出 `69 files, 69 passed, 0 failed`，`ruby -ryaml` 解析 workflow 通过。
