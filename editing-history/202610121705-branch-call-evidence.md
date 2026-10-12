# 具体调用参数复用逐出口证明

Timegrass 的样式组合把 Tag-key Map 与空 Map 的条件表达式传给 `merge-styles`。条件读取 Dynamic 状态时，合并类型变为 `Map<Dynamic,Dynamic>`，已有调用门禁拒绝了两个出口均满足的具体 Map 合同。已在 #2005 保存无 Respo 的最小复现；不是新增 API 或业务类型放宽。

调用关系证明需要边界证据且目标不含类型变量时，复用已有 `expression_is_proven_for` 检查实参各出口。目标含泛型时仍走原来的调用实例化，不借此重新绑定调用者泛型；没有可独立证明的出口仍失败。没有新增诊断或检查入口，也不向应用插入 unsafe。

自查覆盖 `let` 初始化空 Map 后通过 `set!` 改成开放值的情况；复用现有 `may_rebind_local`，可能重新赋值的实参不进入补充出口证明，仍由原始合并类型证明判断，避免陈旧 alias 证据放行。

正常分支和 rest 参数的行为放在 fixture definition 的 `:tests`，通过统一运行器在 native/JS 回放；现有 assertion 脚本覆盖普通 check-only、JS codegen 及 callable 审计中的错误 key / 开放 Map，并确认失败不写 Snapshot、不产生应用产物。原有完整 assertion 回归继续验证调用、返回和泛型边界。

消费者回放只证明对应样式调用解除阻塞，不能作为 Timegrass 完整迁移或浏览器业务验收的替代。
