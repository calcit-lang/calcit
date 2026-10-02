# 顶层不可变值的声明校验

## 依据

#1529 的 Diary 固定基线把 LoginState initializer 声明为 Map<Tag,String>。最小 definition 级复现通过严格检查和 JS 生成，但 `.len` 按错误 Map 声明降低为 map count，native 和实际 Node 24 都拒绝 Struct。print-only 对照确认运行值是 LoginState，而非缺少定义或语法解析错误。#1642 记录独立复现和验收。

## 决策

在公共定义预处理完成后、编译结果发布前，复用共同 TypeProof 检查顶层 `def` initializer 与 schema。只拒绝确定 Mismatch；Unknown/开放证据保持既有迁移政策。不为业务类型或 namespace 增加豁免，不执行 initializer 来获得证据，不改 schema、权限、Dynamic 或方法入口。

复用 `E_SCHEMA_DEF_MISMATCH`：这是声明与定义实现的矛盾，不另造诊断编号。定位原始 initializer owner/path；类型描述使用已有 bounded brief renderer，避免 EnumDef 被显示成 unknown。

完整回归暴露 test-traits 的四个 EnumDef alias 误标为 Impl。实际 `impl-traits` 返回附带方法的 EnumDef，并非 `defimpl` 的 Impl；通过 Calcit CLI 将这四项 schema 修正为 EnumDef，不修改 trait dispatch、顺序或结果的测试期望。

CI 的 WASM 公共定义检查发现 core 的 ReadableByteStream/StreamConsumeError 也把 definition value 错标为 Struct/Enum 实例。通过 CLI 将两项实际 definition schema 改为 StructDef/EnumDef，保留构造限制、Component ABI 和语义测试；不让 definition/instance 在 TypeProof 中混为兼容类型。

WASM 回归也暴露已有 attachment 推断把表达式位置的命名 StructDef/EnumDef 的 TypeRef 当成实例。在 attachment 的共同推断中，使用实际输入表达式的源定义区分 definition value 与指向同名 nominal type 的实例 schema，保留 StructDef/EnumDef 及公共 definition identity。不扩大普通 query 的表示变更，不改正确的 StructDef schema，也不豁免该诊断。已有 typed-string-conversions runner 回放 trait 方法到 JS/WASM 验证这一分层区别。

直接跨 namespace 读取同名 alias 另暴露 JS emitter 的绑定冲突，已单独建立 #1643，失败 Snapshot 保留在本地 `.calcit/snippets/def-value-js-collision.cirru`。本 fixture 通过普通 `:as reader` namespace alias 验证跨模块值，不改 JS 路径或产物；不把此 fixture 通过宣称为 direct qualified reference 的绑定问题已修复。

共享 fixture 的数据定义放在 `app.values`，由 `app.main` 和 `app.reader` 正常引用，避免测试自身制造 eager ESM value initialization cycle。负例同时修改真正 constructor owner、两层本地 alias 和另一 namespace 的 alias，不能仅检查入口本地 schema。原循环/同名绑定复现保持在独立问题中，不通过调整原始复现宣称 JS emitter 已正确。

## 验证组织

Calcit fixture 的 `:tests` 验证 nominal 值、本地及跨 namespace alias、Map、Number、String 和明确 Dynamic 保存。既有 assertion runner 回放同一 Calcit 验证函数到 native/JS，并检查矛盾在各 target 的 preprocessing 阶段被拒绝；不声称 WASM 已支持这些 nominal 值的运行。

Rust CLI 测试只覆盖 schema mutation、严格/keep-going 检查、JS 生成拒绝、原始 Snapshot 不变与无新增输出产物。正式提交前仍须完成 fmt、Clippy、完整 Rust/JS 集成和真实消费者验证；单个通过或本历史文件不是全部门禁成功的证明。

## Snapshot 缓存发布路径 review 修复

完整 review 指出 `compile_source_def_for_snapshot` 直接发布编译缓存，未经过新的 initializer
证明。代码核实该路径被 Snapshot/query/type coverage/schema synthesis 使用，后续 ordinary
preprocessing 的 cache hit 会直接返回，确有遗漏。该入口复用同一个校验 helper 后才发布，
不复制类型规则、不把消费者的检查结果或声明当成证明。新增低层测试检查错误没有进入缓存，
后续普通入口仍拒绝错误，合法 Number 与显式 Dynamic 缓存仍可重用；共享 Calcit 语义 fixture 保留。
低层回归加载真实 embedded core，确保 `def` macro 实际展开而不是缺失 core 时留下未解析的表达式。
同一测试在移除 Snapshot 校验时实际错误成功（测试失败），恢复校验后通过；错误不入缓存，
随后普通入口仍拒绝，合法 Number 与显式 Dynamic 保持缓存复用。完整验证仍须通过，
不把定向测试当成新 HEAD 的完整验收。
