# effects-graph 读取项目与模块定义的 `:tags`

关联 #1567、#1566。#1947 之后 effects-graph 只按声明分类效果，但声明只覆盖 core 与宿主 proc；项目和依赖模块（calcit.std、js-ffi）定义上的 `:tags` 没有进入 `ProgramDefEntry`，无法作为显式声明被读取。

## 决策

`ProgramDefEntry` 新增 `tags` 字段，加载 Snapshot 时取自 `CodeEntry.tags`，热更新 `changed_defs` 时沿用原有 tags。effects-graph 在分析一个定义时，对其调用的非 `calcit.core` 定义读取 tags，经与 core 相同的 `tags_to_effect_kinds` 映射后在调用点记录，目标写 `ns/def`；没有效果 tag 的定义（包括只带 `:deprecated` 等非效果 tag）调用点仍不记录。这复用既有 `:tags` 数据，不引入新语法，也不新增 `query` 字段或诊断编号。

## 兼容与剩余

输出 envelope 不变，仅在被调用定义声明了效果 tag 时多出 effect 项。calcit.std 与 js-ffi 中带效果的定义仍需在各自仓库补 `:tags`，补齐前它们按调用图子节点分析，未声明处不会被猜测为效果。

只改 `:tags` 而函数体不变的编辑不会出现在热更新的 `changed_defs` 中，该变化在完整重新加载后才生效。

## 验证

`calcit/test-effects-graph.cirru` 新增 `declared-helper`（纯函数体、`:tags #{} :effect`）与 `declared-caller`，`fixture_reports_declared_effects_and_unknown_targets` 断言调用点记录 `effect`、被调用定义自身无效果。

补充测试：`deprecated-only`（仅非效果 tag）、`mixed-tagged`（`:deprecated :log`）、另一命名空间的 `test-effects-graph.lib/lib-effect`（`:file`）；`src/program/tests.rs` 覆盖热更新保留已有 tags、新增定义 tags 为空。效果项按 kind、target 排序，输出稳定。
