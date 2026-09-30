# 已知不兼容的局部类型断言

关联 #1586 与 #1538。官方 0.28.0-alpha.2 默认严格模式接受
`let ((x |hello)) (assert-type x 'Number)` 并返回 String；加法仍通过预处理，直到 runtime 才报错。
这是局部 assertion 直接覆写 scope evidence 并被擦除造成的确定错误，不是 Dynamic 默认政策决策。

复用已有有方向的 `prove_with_bindings`，仅在 `TypeProof::Mismatch` 时发出
`E_ASSERT_TYPE_MISMATCH`，在修改 scope 之前失败。保留 actual/expected、local 名和源码位置，
给出修正声明或边界 decoder 的提示；不执行值、不添加新 registry、不用 warning 数量做门禁。
`NeedsBoundary` 与真正 Proven 仍不同，本批不宣称修复所有开放 assertion、callback 或返回值漏洞。

回归发现原 TypeProof 部分 Mismatch 实际表示缺失证据，而不是类型矛盾：
Number→numeric refinement 仍需要范围检查；开放 struct/enum 缺少名义身份；Optional/JsNullish
到具体值需要非空证据。共享关系将这些情况保留为 NeedsBoundary，不变成 Proven，
旧 compatibility 的隐式数值/非空收窄仍不允许。嵌套 payload 确定不兼容继续是 Mismatch。
这不是按 core 或 Respo 名称添加例外；真实 consumer 的严格检查用于防止误伤，
原 nominal-slot 方向性测试继续确认开放 enum 不能证明具体 slot、不能提交 binding，
只把“缺身份”与“确定矛盾”的诊断分类区别开。

Calcit core `assert-type` definition 新增三个正例：同型 scalar、闭合 List、宽断言保留 concrete evidence。
Node 从 Snapshot 读取这些 AST，复用 native/JS，并仅对已有支持的 scalar 子集运行 core WASM。
四个已知不兼容的表达式分别验证 native/check-only/JS/WASM 在预处理阶段拒绝，不能到 runtime 才失败。
已有 Rust scope metadata 测试把错误的 Number→Fn relabel 改成合法 Number→Number，
仍核对内部 local/scope 传播；不新增 Rust 语义测试取代 Calcit 测试。

该 bugfix 在 0.28 收尾，broader Dynamic/Unknown proof 和消费者默认翻转仍按 0.29 分阶段计划推进。
完整仓库门禁及真实消费者证据记录在 PR，不把单个错误码或测试通过当完整类型系统安全证明。
