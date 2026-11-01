# Union 名义别名模式

Regex 的 0.34.0 迁移需要 `String | Regex`，公开 `Regex` 构造器是附带 trait 的 base StructDef 别名。旧 union 检查只比较模式短名字，成员解析却已经沿别名找到原定义，因此拒绝同一个合法别名。无 FFI 的 Base/Alias 最小复现确认该不一致，见 #2529。

保留原成员名标签；额外拼写通过现有模块 import 和 TypeRef 解析，使用已有 struct/enum 的名义定义比较。不同定义即使字段相同也不能匹配；两个指向同一成员的名字仍触发原重复分支检查。运行时仍使用该成员原有的名义测试，不增加值包装、类型放宽、诊断码、proof 或迁移 fix 规则。

正向行为在 `calcit/test-union.cirru` 的 definition tests 中覆盖 struct 与 enum 别名，通过现有共享运行器回放 native/JS，并加入原 check-core-tests 入口。无法运行的错误程序留在原 edit CLI 集成测试中，验证不同名义定义与重复别名分支的拒绝。

官方 0.34.0 的新附带测试失败；修复候选通过同一测试，真实 Regex 候选的 26 个定义与两个原生附带测试、原 FFI trace 入口也通过。此候选尚未发布，完整核心门禁和最新 CI 结果以 PR 为准，不能作为官方版本验收。
