# 默认预检查加入断言证明审计（#2121）

普通 `--check-only` 会放行 `Dynamic` 流向具体返回值或具体参数的写法，只有 `calcit strict`
用断言证明能发现。#2121 的五个复现（p03/p12/p21/p30/p31）都属于这一类。

改动：`with_default_proof` / `check_with_default_proof` 在严格诊断开启时把检查包进
`with_assertion_proof`，并像 strict review 一样丢弃宏所拥有定义上的证明告警。接入点：
check-only、`test`、直接运行与 `js` 的预检查（运行/代码生成前先做一次不输出横幅的 check-only），
以及 `calcit strict` 与 public API 检查。审计恢复编译结果，运行与代码生成仍用普通输出。
`--compat-types` 保留旧行为。

随之变化（与 `calcit strict` 一致）：`:js-ffi` 适配器转换到非 host 目标（`unsafe-coerce raw 'String`）
不再证明返回值，报 `E_FN_RETURN_UNPROVEN`；泛型关系被具体回调收窄时默认检查先报
`E_CALL_ARGUMENT_UNPROVEN`。相关三个旧测试改为断言默认检查的新结果，同时用直接的普通 strict
预处理保留原有诊断断言。生态 26 个仓库默认模式全部通过。

回归：`default_proof_rejects_directional_dynamic_flows`（五个复现，strict 下报对应错误码、
compat 下通过）与 `default_proof_accepts_explicit_dynamic_schemas`。

English: the default preflight for check-only, test, run and js now runs the assertion-proof
audit with strict-review semantics; `--compat-types` keeps the previous behavior.
