# sort-by 回调上下文修复进度（#2020）

Timegrass 的排序回调在严格调用证明中丢失 List 成员类型。完全具体的 `List<Map<Tag, Number>>` 也能复现，因此不能用应用断言、业务默认值或扩大 Dynamic 绕过。

当前实现先解析 inline fn/defn 的构造器身份，再复用 core 已有的 selector 参数关系提供函数体检查上下文。方法通过实际 impl 的 core 定义复用相同关系；不凭方法拼写或相似 Fn 签名猜测排序语义。具名 inline defn 与 macro 展开的 fn 都应从函数体推导返回值，调用方的临时输出变量不成为词法刚性声明；显式 hint 及权限边界保留。

严格反例要求回调输入不得反向收窄开放成员。最终校验复用现有 receiver-specialized 关系，仅补接实际专门化的回调合同，不将非回调 API 的运行时失败语义改成编译期拒绝。初次全 core 回放暴露的 assoc/contains? 非数值索引和 Option 短路回归已据此收紧修复范围，原测试保留。

语义用例位于 `app.sort-selector` 的 definition `:tests`，严格负例扩展已有 `check-known-assertion.mjs`；没有新增脚本、命令或诊断编号。Snapshot 使用正式 CLI 的 dry-run 与 scoped-revision 事务写入。

本检查点尚未完成：局部别名 `let order &list:sort-by` 仍只保存通用 Fn 合同，缺失可验证的 callee 来源。相同签名不代表相同实现，不能给任意 Dynamic selector 参数猜测 List 成员关系；还需正确处理 shadowing 与重绑定。该失败测试继续保留，完整 proof runner 仍为红，不能据其他测试通过宣称 #2020 完成或合并。

检查点验证：8 项新附带测试在 native/JS 全部通过；直接、线程、两种方法语法及具名 inline defn 的证明通过；开放行、开放数值字段、收窄回调、调用方刚性泛型共 12 个负例按预期拒绝。完整 core 回放 native 633/633、JS 632/632、WASM 403/403 通过，原排除项不变；Rust lib 1003 通过、1 项原有忽略，Clippy 通过。proof runner 随后在局部别名处失败，后续部分尚未回放。

## 局部别名补充

后续实现复用 `explicit_open` 的词法绑定记录保存已经解析的函数值，不新增平行来源表。参数及同名绑定自然遮蔽旧记录，离开作用域恢复；别名复制保存独立的函数值来源。显式合同改变或写入使该来源失效，不能只凭相同的开放 Fn 签名继承某个 core 实现的特化关系。

一旦严格调用证明使用了这个实现来源，后续 `set!` 保守拒绝，保护捕获该变量的闭包与循环中的后续调用；这不是完整的可变函数流分析。普通不依赖实现关系的调用，以及写入原变量前复制出的独立别名，保留原行为。使用已有 `E_CALL_ARGUMENT_UNPROVEN` 诊断，限制同时写入 List 用户文档。

隔离 Snapshot 的新增回放：10/10 native、10/10 JS，另 2/2 JS 赋值边界通过；6 个别名遮蔽、无关实现和前后重绑定负例均在预期边界拒绝。旧基线完整 proof 与 cargo test 尚未完成时，上游又合并 String Option 与 CLI format 修复，因此主动停止旧轮次，不算全量通过；同步上游后的最终组合须重新完整验证。

## 区分回调与被保存的函数值

同步 main 后完整 Rust 测试通过，CI 的 Struct 写入回归随后发现 `assoc` 更新 Fn 类型字段会被新校验提前拒绝：仅用“专门化参数中包含 Fn”识别回调不成立，存储函数值并不等于调用该函数。这里将已有 fold / sort selector 关系集中为私有的 callback specialization helper，并由原通用 specialization 复用；早期上下文、方法和最终回调校验只接入这个语义边界。Struct 写入仍由原字段合同证明处理，保留原结构化 `E_CALL_ARGUMENT_UNPROVEN`，不改成其他诊断或放宽门禁。既有 `check-struct-field-js.mjs` 用例不删、不改期望，只补空 stdout 的明确报错，避免 JSON parse 错误掩盖根因。

此修复后的完整 Struct 回归与排序正负例定向 runner 均通过，Clippy 通过；完整 Rust / check-all 重新回放，不沿用旧 HEAD 的成功结果。
