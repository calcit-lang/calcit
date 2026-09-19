# 登记未引用的测试工具（#1211）

- `scripts/release-manifest.test.mjs` 之前没有 runner。在 `.github/workflows/test.yaml` 的
  `core_checks` 增加 `node --test scripts/release-manifest.test.mjs`，纳入 CI。
- 在 `scripts/wasm-validation.md` 新增“手动渐进套件（不在 CI 中）”章节，登记
  `test-wasm-suite.sh`、`test-wasm-suite-extended.sh`、`test-wasm-run.mjs`、`test-wasm-call.mjs`
  的用途、与默认门禁 `test-wasm.sh` 的区别，以及 `CALCIT_BIN` 运行示例；相关 fixture
  `calcit/test-wasm-suite.cirru`、`calcit/test-nil.cirru` 也随之有明确入口说明。
- 未删除这些脚本：它们近期仍在维护，属于有意的按需工具；本改动只补上明确去向。
- 验证：`node --test scripts/release-manifest.test.mjs`（2 pass）、workflow YAML 解析通过、
  `CI=true CALCIT_DOCS_CHECK_JOBS=4 bash scripts/check-docs-md.sh` 仍 69/69。
