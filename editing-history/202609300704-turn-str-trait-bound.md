# `turn-str` 兼容入口的类型边界

关联 #1456。此前 `turn-str` 标为无约束的 `T -> String`，函数体直接调用 `turn-string`。因此 `turn-str ([] 1)` 能通过严格预处理，却在运行时因 List 不受支持而失败；即使名义类型实现了 `ToString`，这个旧入口也不会使用其方法。

现在 `turn-str` 保留为内部兼容名字，签名与首选 `to-string` 共用 `T: ToString -> String`，实现转调 `to-string`。这让已有六类标量保持相同文本，并让自定义名义实现按 trait 选择；没有证明 `ToString` 的集合、Unit 和开放 Dynamic 在预处理阶段被拒绝。精确查询旧名仍会提示迁移到 `to-string`，但它不再作为 Agent 推荐的公开入口。

Calcit 定义测试覆盖六类标量和自定义 Struct 实现；脚本确认负例在严格检查阶段失败，并核对生成 JS 的标量及名义 trait 分派。旧代码若依赖先通过检查再在运行时失败，应改用显式匹配、受检转换或诊断格式化，而非长期使用无约束别名。
