# Enum schema 别名的 type-slot 关系

问题由 Protea 迁移报告的 #2447 引出。当前 main 的最小对照进一步证明，即使入口槽绑定到显式声明 Enum schema 的代表值，传入 Enum 回调也可能失败：TypeRef→AnonymousEnum 的名义类型快捷分支在无法解析名义 Enum 时提前返回，未进入已有的 schema 别名展开。

本次只让已确认的名义 Enum 走快捷分支，其余引用继续走原别名解析与递归守卫。同一调整应用于兼容性比较与严格证明。不会执行代表值来推断类型，不把缺失 schema、Dynamic、未绑定槽或错误家族当作 Enum 证据；匿名 Enum 也不能证明特定名义 Enum 身份。

可观察语义放在 tests/fixtures/enum-slot-alias.cirru 的 :tests，沿用现有 native/JS 回放。strict_check_cli 验证真实入口的完整检查与正常执行，以及错误家族、Dynamic 和未绑定槽的拒绝。无 schema 代表值的绑定策略仍由 #2447 跟踪，本次不宣称整个 issue 已解决。
