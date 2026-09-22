# 排除引号中的 Dynamic 定位误报

`analyze weak-types` 的 `code-dynamic` 源码 inventory 曾把 `quote` / `quasiquote` 中作为数据的 `:dynamic` 当作活动类型位置，与同一扫描器对 `nil` 和 `unsafe-coerce` 的处理不一致。

现在复用现有 quote context 跳过这些数据节点，同时保留 `~` / `~@` 后重新进入活动代码的 Dynamic，以及普通 `assert-type` 类型位置。此改动只影响迁移定位报告，不改变预处理诊断、类型关系或编译语义。
