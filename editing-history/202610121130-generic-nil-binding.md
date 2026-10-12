# 泛型绑定不再隐式构造 nullable

正式 0.29.0 会拒绝 `= 1 nil`，而 variadic bottom 精化接入后的 main 会接受。原因是 rest 参数开始匹配原始泛型，旧兼容关系在遇到 Nil 与具体类型时会把已经确定的 T 改成 Optional<T>。这不是没有 payload 的 Never 精化，而是扩大实际允许的值集合。

删除兼容关系正反两个方向的 Nil 合并分支，让已有绑定继续通过普通类型关系检查。保留 `can_refine_bottom_binding`，保留已明确声明的 Optional/JsNullish 边界；不增加调用点白名单、诊断或 Dynamic 退路。该关系同时供固定参数、rest、proc 和方法使用。

用户语义通过已有类型推断 Snapshot 的 definition `:tests` 和共享运行器验证：空 Option/List 精化、nil 的同型固定与 rest 调用。既有严格调用检查增加 Number/String/Option 与 nil 的双向拒绝，并保留 Ref invariance 和错误 payload 负例。原 Rust nullable 合并测试改为检查失败后绑定不变，覆盖内部双向绑定状态，而不新增用户语义的平行测试框架。

完整验证结果写在关联 PR；该修复不代替消费者迁移和正式发布门禁。
