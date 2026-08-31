# Standalone Calx benchmark cutover / 独立 Calx benchmark 切换

- Remove the release runner, Node orchestration/settings policy, machine-specific report archive, and benchmark-product contracts only after calcit-calx-bench#3 pinned merged Calcit `42c2f339`, passed Ubuntu/macOS CI, and completed the 182-sample clean-state matrix with correctness true.
- Retain `calcit::codegen::calx::benchmark_session` edition `calcit-calx-benchmark-session/1`, Calx lowering, revision-safe cache, trap/fallback, differential correctness, and authoritative scalar fixtures in core.
- Standalone reports own performance provenance and sampled crossover conclusions. Core tests own deterministic semantics; sampled timings never become a correctness or automatic offload policy.
- Replace local report/methodology references with stable links to `calcit-lang/calcit-calx-bench`, keeping short docs entries for `calcit docs read/search` discovery.
- Measured sequential delta after the standalone caps cutover: binary targets `3 → 2` and direct dependencies remain `30`. A fresh rebase-after-caps release build keeps the `calcit` binary byte-identical at `9,516,000 → 9,516,000` bytes; removing a separate runner target does not change runtime payload.
- The cutover removes 13,500 lines and adds back 89 lines of ownership/discovery documentation. Eleven Rust runner/product-contract tests and four Node settings tests move out of core; the 621 core semantic tests remain green.
- Default `check-all` no longer runs the four benchmark-settings tests or owns machine-dependent policy. The standalone repository supplies six Rust tests, eight Node/pin tests, dual-platform CI, quick smoke, and the full 182-sample correctness-gated matrix.
