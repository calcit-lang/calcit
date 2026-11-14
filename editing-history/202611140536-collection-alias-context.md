# 集合类型别名的字面量推断

集合字面量从期望类型获得成员信息。期望类型是 `deftype` 别名时，编译器先展开别名，再沿用 List、Tuple、Set 和 Map 的成员推断。例如返回 `Palette`（`List Color`）时，`[] :red :green` 的每项按 `Color` 检查。

按原有规则检查构造器身份、Tuple 长度、Map 键值数量和展开参数。普通 Tag 或 Dynamic 位置仍按原类型处理。定义测试验证枚举列表与泛型 Map 别名，CLI 回归验证拼错枚举分支的诊断。
