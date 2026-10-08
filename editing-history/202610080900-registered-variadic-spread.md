# 注册宿主 proc 的变长 spread 复用 arity 证明

关联 #1694。calcit.std 0.2.37 副本升级到当前 main 后，`fix --workflow strict --verify` 只剩 `calcit.std.path/join-path`
的 `&call-dylib-edn (get-dylib-path ...) |join_path & xs` 被 `spread-call-proof-v1` 列为待审。

原因有两处：注册 proc 没有静态 Fn 类型，`typed_rest_spread_call_is_proven` 直接返回 false；该调用又是项目 macro
`decode-string` 的实参，规划器先要求“可改写的源码上下文稳定”，使已证明判断根本不执行。

决定：注册 proc 只按描述符判断。显式前置实参满足 `arity_min`、没有 `arity_max`、唯一末尾 spread 实参静态为 List 时，
该调用与等价的固定 arity 调用受同样的（无元素）合同约束，视为合法变长调用。“已合法、无须建议”只需要唯一的编译器
source-expression 与非 macro 调用头，不再要求改写稳定性，因为不写回任何内容；字面量展开的可应用改写仍要求稳定上下文。
不新增名称白名单、诊断、规则或 CLI 入口。

保留边界：固定参数不足、Dynamic/未知 spread 实参、macro 复制实参导致表达式不唯一时仍为 `requires-review`。
验证见 `tests/fix_cli.rs` 的 registered variadic 用例（规划器行为无法用 Calcit `:tests` 表达）以及 PR 中的 std 副本回放。
