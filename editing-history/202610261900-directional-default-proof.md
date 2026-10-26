# 默认预检查加入断言证明审计（#2121）

普通 `--check-only` 会放行 `Dynamic` 流向具体返回值或具体参数的写法，只有 `calcit strict`
用断言证明能发现。#2121 的五个复现（p03/p12/p21/p30/p31）都属于这一类。

改动：`with_default_proof` / `check_with_default_proof` 在严格诊断开启时把检查包进
`with_assertion_proof`，并像 strict review 一样丢弃宏所拥有定义上的证明告警。接入点：
check-only、直接运行与 `js` 的预检查（运行/代码生成前先做一次不输出横幅的 check-only，结束后恢复编译状态），
以及 `--keep-going`/`--all-defs` 的 strict check。审计恢复编译结果，运行与代码生成仍用普通输出。
`--compat-types` 保留旧行为。

随之变化（与 `calcit strict` 一致）：`:js-ffi` 适配器转换到非 host 目标（`unsafe-coerce raw 'String`）
不再证明返回值，报 `E_FN_RETURN_UNPROVEN`。相关两个旧测试改为断言默认检查的新结果，
同时用直接的普通 strict 预处理保留原有断言。

审计只在普通检查通过后运行：普通检查能拒绝的代码仍报原来的诊断（例如
`E_ERASED_GENERIC_RELATION`、`W_FN_RETURN_TYPE_MISMATCH`），js 编译失败仍生成
`calcit.build-errors.mjs`。`eval` 与 public API 检查（`--check-only --ns`）不做审计。
生态 26 个仓库默认模式全部通过。`calcit test` 暂不做逐项审计：试过之后生态里 phlox、respo、memof 的测试体被拒（测试里的开放写法），留作后续。

新增 `--no-proof-audit`：只跳过审计，其余严格诊断照常，与 `--compat-types` 互斥。仓库内只有
`calcit/test.cirru`（经模块带上 `test-algebra.cirru`）依赖审计拒绝的写法（`update x 0 inc` 把
具体回调传给 Dynamic 回调参数、`assert-type` 收窄 Dynamic 等），CI 与 package.json 中运行它的
地方加上该开关。

回归：`default_proof_rejects_directional_dynamic_flows`（五个复现，strict 下报对应错误码、
compat 下通过）与 `default_proof_accepts_explicit_dynamic_schemas`。

English: the default preflight for check-only, run and js now runs the assertion-proof
audit with strict-review semantics; `--no-proof-audit` skips only the audit and `--compat-types` keeps the previous behavior.
