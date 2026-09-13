# 突出冗余 `do` 的检测与修复流程

Issue: #1061。

本次整理不新增 compiler warning，也不改变 `redundant-do-v1` 的语义边界。核心认识是：`calcit fix`
的 preview 本身就是问题写法检测入口；用户需要在升级清单中看到它，并能从检测结果安全过渡到带
Snapshot revision 的 apply 和第二次幂等复查。

文档现在明确区分两类位置：`defn`、`fn`、`let` 等 variadic body 可 splice 冗余 `do`；`if` 分支、
调用参数、binding value、`defmacro` 和 quoted data 必须保留现有单表达式或语法树边界。升级手册同时
更新当前稳定工具组合为 Calcit 0.14.16 与 calcit-caps 0.1.1。
