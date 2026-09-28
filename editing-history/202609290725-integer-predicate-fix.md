# `round?` 函数引用的显式迁移

## 证据与限制

`integer?` 已公开 `Number -> Bool`，其实现直接委托经跨目标验证的 `round?`。Cirru reader 在 preprocess 前就把未限定 `round?` 解析成内建 Proc，因此普通定义引用追踪拿不到它的 source location。本批只在可回溯的源码中改写单参数 `round?` 调用头；定义中若有同名词法绑定则整项跳过，避免将参数或 let 绑定形状误当成调用。quoted 数据与非调用引用不改，未知宏上下文仅提示人工审阅。替换成全限定 `calcit.core/integer?`，不会被局部或 import 同名定义截获，也不改变实参位置、求值次数或失败命题。

Number `.round?` 到 `.integer?` 虽有相同公开类型，但当前指向不同实现节点；不能套用集合方法“同实现同契约”的证明。本批不自动改写方法形式、开放接收者、未知 macro 或附带的 `:tests` / `:examples`。后续若支持方法迁移，须单独证明静态 receiver、调用语义和实现委托关系，不能只按叶子拼写替换。

## 验证

复用现有 `calcit fix` 的预览、revision、事务应用和幂等机制；CLI 测试覆盖普通 builtin 调用、quoted 数据、未知 macro、方法保持原样、stale revision 与执行前后定义测试。规则显式选择，不加入旧 preset。
