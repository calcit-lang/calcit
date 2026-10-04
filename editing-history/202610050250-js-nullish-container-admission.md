# 容器中安全加入 JavaScript nullability

实际 registry 安装的 0.29.0-alpha.5 在已合并 Respo #221 的原 106 tests 中通过 100 项；正式 0.28.0 通过 106 项。多个失败来自将具体 EventHandler 或 nil 的事件表存入 `Map<Tag,JsNullish<EventHandler>>`，不是源代码需要扩大 Dynamic。

最小 Calcit definition-attached 用例确认 Map/嵌套 List 的具体 Number、nil、named callable alias 和显式 Fn 签名均失败。根因是通用 proof 将引入和移除 JsNullish 一律标记为 LegacyNullish，constructor 的特殊处理只覆盖最外层。

在通用 proof 的迭代路径与 binding-aware 路径中，允许 Nil 加入 JsNullish，允许已证明的 payload 加入 JsNullish；容器递归、函数 variance 和 Ref 的双向证明复用原关系。移除 JsNullish 仍保留边界，历史 Optional 的关系不变，不改 runtime 值、不改 source lowering、不增加诊断编号或公开命令。

四组语义用例保存在 test-struct 的定义 `:tests`；现有 check-known-assertion 读取原 AST 在 native 和真实生成 JS 重放，并验证错误 payload、键、callback 输入/输出/arity、擦除 Fn、Dynamic、nullable elimination 与 mutable widening 的编译拒绝及 Snapshot 不写回。Respo 回归准确记录剩余 branch join 与混合 literal 问题，不据此宣称整个迁移完成。关联 #1768、#1529、#1694、#1553。
