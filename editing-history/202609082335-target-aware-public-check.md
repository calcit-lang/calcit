# Target-aware public definition checks / 按目标检查公开定义

Issue: calcit-lang/calcit#874

## English

- Added `calcit analyze check-public` to preprocess every definition in one or
  more exact public namespaces without running the application entry or host
  effects. Calcit has no private-export marker, so functions, values, macros,
  data declarations, traits, and implementations in the selected namespaces
  are all included.
- Required an explicit target on the selected entry and validated definition
  `:ffi :target` metadata before preprocessing. Wrong runtime targets, malformed
  metadata, missing or empty scopes, and unapproved dependency namespaces fail
  closed.
- Added a versioned JSON report with deterministic scope revision, checked
  definition IDs, per-definition status, diagnostics, completeness, duration,
  and summary-only support. Any preprocessing warning or error returns a
  non-zero status.
- Preserved ordinary `--check-only` reachability semantics and added a
  regression proving an unused invalid public function is missed there but is
  caught by `check-public`.

## 中文

- 新增 `calcit analyze check-public`，在不运行应用入口和 host effects 的
  前提下，预处理一个或多个明确公开 namespace 中的全部 definitions。Calcit
  没有独立 private export 标记，因此函数、值、macro、data declaration、trait
  和 implementation 都纳入检查。
- 要求所选 entry 显式声明 target，并在预处理前验证 definition 的
  `:ffi :target`。错误运行目标、畸形 metadata、缺失或空 scope，以及未显式
  允许的 dependency namespace 均 fail closed。
- 新增版本化 JSON 报告，包含确定性 scope revision、实际 checked definition
  IDs、逐 definition 状态、diagnostics、完整性、耗时和 summary-only 支持；
  任一预处理 warning/error 都返回非零。
- 保留普通 `--check-only` 的可达性语义，并用回归测试证明未引用错误公开函数
  会被普通入口检查漏过、但会被 `check-public` 捕获。

## Validation / 验证

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` after rebasing onto main at `00ea1c27` (779 library + 315 CLI
  + 23 wasm passed)
- `yarn compile`
- `yarn check-agent-interface` (21/21 scenarios passed)
- Calcit 0.14.4 reproduction: ordinary js-ffi Node `--check-only` returned 0
  with an injected unused bad function; the historical temporary-reference
  checker failed with `W_FN_ARG_TYPE_MISMATCH`.
- Candidate js-ffi checks: Node 85/85 definitions and browser 114/114 passed;
  the inventories included 18 and 34 data/trait rows respectively. A Node entry
  checking the browser namespace failed before preprocessing with
  `E_JS_FFI_TARGET_MISMATCH`.
- Release-build preprocessing duration across three warm process runs was
  10.47–11.38 ms for Node and 18.00–18.34 ms for browser. Full JSON output was
  15,206 and 21,557 bytes; summary-only output retaining checked IDs was 2,868
  and 4,120 bytes.
