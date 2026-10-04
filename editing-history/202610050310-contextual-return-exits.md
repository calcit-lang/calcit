# 声明返回合同按独立退出值检查

Respo 的 `normalize-ref` 与浏览器 `query-mount-target` 分别返回已证明的回调/宿主值或 nil，但底向上推断合并成内部 Optional 后被 JsNullish 返回声明误拒绝。保留既有 nil RFC 的 Optional/JsNullish 区分，不修改类型关系或给内部合并类型添加宿主授权。

在现有返回检查方向中验证 `if` 和 `match` 的每个退出值、尾部 `let` 的父作用域初始化值及别名。别名证据仅在当前返回合同检查期间存在；不会更新声明、推断、program cache 或诊断 provenance。绑定和 pattern payload 遮蔽同名证据。开放泛型仍走既有替换路径，遍历有节点和深度预算，recur 仍需已有词法检查和独立退出证明。

用户语义放入 `calcit/test-struct.cirru` 的 `:tests`，在现有 `check-known-assertion.mjs` 回放原 AST 到 native/JS，并验证错误 payload、Dynamic、遮蔽、错误 callback 和 recur-only 的拒绝。普通编译保留既有开放返回迁移策略，显式 proof audit 不把开放声明当成证明。Respo 源码及测试不修改；混合 callback/nil Map 成员仍属剩余问题，不随本次返回修复宣称完成。
