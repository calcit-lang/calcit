# `--check-only --ns` 运行默认证明审计

#2506：两份下游报告（Wanderlist 的 `dissoc-in` 返回、Calcium Workflow 的 `render-app!` mount-target 参数）都是 `--check-only --ns` 报 `complete=true`、diagnostics 为空，普通入口 `--check-only` 却报 `E_FN_RETURN_UNPROVEN` / `E_CALL_ARGUMENT_UNPROVEN`。原因是 public check 逐个调用 `ensure_ns_def_compiled`，没有进入 #2121 的默认证明审计，`complete` 只代表“全部做了普通检查”。

维护者 2026-11-01 决定：0.34.0 起 `--ns` 与普通 `--check-only`、`--keep-going`、`--all-defs` 一致，普通检查全部通过后再对 passed 的定义跑一遍证明审计（`check_with_default_proof`）。这是非 patch 版本允许的新增阻断诊断，升级说明在 `docs/run/upgrade.md`。

兼容边界：

- 普通检查失败时只报告普通诊断，不叠加审计结果，与 `check_entries` / `strict_check` 相同。
- 审计期间 macro 实现自身的 warning 在决定行状态前丢弃；审计重新展开的 macro 输出静默，避免重复打印。
- 证明审计只覆盖项目命名空间（`PROOF_PROJECT_ONLY`），`--deps` 选入的依赖仍只做普通检查，与其他入口一致。
- `--no-proof-audit`、`CALCIT_PROOF_AUDIT=0` 跳过审计；`--strict-types` 的 entry 预检不变。

验证：`public_check_runs_default_proof_audit_after_ordinary_check` 先断言普通 preprocess 接受 `Fn Dynamic (-> Number)` 的 `as-number`，再断言 public check 因审计失败；`scripts/check-strict-default.mjs` 的 macro 只展开一次断言保持通过；#2506 的最小复现在 `--ns` 下报 `E_FN_RETURN_UNPROVEN`，加 `--no-proof-audit` 恢复通过。
