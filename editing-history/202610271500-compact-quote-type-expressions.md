# 精简 quote 类型表达式（#2433）

`assert-type`、`unsafe-coerce`、`js-cast`、`parse-cirru-edn-as`、`decode-map-as` 的类型参数和
`calcit edit schema --code` 接受 `quote $ Fn Number (-> String)` 这类精简写法。它只是表层语法：
预处理和 schema 写入入口先把它展开为规范的 `:: 'Fn $ {} …` 代码形式，Snapshot 存储、类型检查器、
JS 运行时类型匹配和 WASM lowering 都不变。

选型依据记在 issue 中：生态 16,254 条 schema 的精简写法约为 EDN 的 0.60；
`calcit cirru format` 对箭头写法会折出 `, -> …` 续行，所以返回值改为一个 `(-> T)` 子句，
其余属性用 tag 开头的子句。名字规则把未限定的小写叶子和符号叶子留给运算符，
`:generics` 列表项、参数化 trait、`.`、`=`、`forall` 都先报错保留位置；未知子句报错，
避免新版本写入的子句在旧版本中被静默忽略。

以前 `quote $ Banana Number` 会落入 `from_calcit` 兜底，被当作 `List<Dynamic>` 静默通过；
现在它按 `Banana<Number>` 检查，语法错误报告 `E_TYPE_EXPR_SYNTAX`。新编号的原因：
现有编号都描述类型关系或 schema 存储，没有“类型表达式本身不合语法”这一类。

`calcit edit format` 的旧 tag 写法规范化会给列表补 `::`，这会把精简写法改坏，
因此遇到 `quote <精简 payload>` 时整体跳过。

验证：`cargo test --lib type_expr`（含 core 全部 539 条 schema 的精简打印 → 写入 → 还原往返），
`cargo test --bin calcit type_fail_compact`，`test-types.cirru` 的 `:compact-type-expr` 测试在 native/JS 运行，
以及改写为精简写法的 test-generics/enum/traits/struct 用例。
