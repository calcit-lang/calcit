# 环境读取结果的独立证据

## 问题与决策

正式 `0.29.0-alpha.10` 中，Diary 完整 strict workflow 的 16 个诊断包含 core
`get-env` 的返回证明失败。无模块的同形 wrapper 同样被拒绝：实际 raw 环境读取只
返回 String/nil，底层 proc 的宽签名却同时覆盖任意 legacy fallback，因而一参数
调用丢失 String 证据。普通编译或外层 `Option<String>` 声明不能替代实现证明。

在现有 proc 调用推导中按实际 arity 恢复结果。名称必须独立证明为 String；无
fallback 返回内部 nullable String。有 fallback 时，将真实 String 分支与该参数
独立证据通过既有分支 join 合并；nil 保留缺失可能，String 可形成 String，开放
或异构输入保留开放结果。参数先求值；若其证据是 Never，不把它当作可选的惰性分支。
兼容性 join 后，还要求两个独立 producer 都能证明合并结果；普通 String 与名义
Option 默认值不能仅靠历史兼容关系被合并成实际并不存在的 Option 包装。

这是内部 synthesis 的精度修复，不是新增表层 overload 或 fallback 类型限制。
raw proc 的一等宽签名与 native/JS/WASM 实现不变；没有新诊断、迁移规则或 runtime
ABI。公共测试 wrapper 使用 nominal `Option`，不恢复已退场的公开 `Optional` schema。

## 回归与边界

共同语义放在 core `get-env` 与现有 `test-struct` definition 的 `:tests`，覆盖
缺失、存在文本、空字符串、UTF-8、nil/String/Number/开放默认值，以及默认表达式
的 eager 求值和一次副作用。现有 known-assertion runner 回放同一 AST，向宿主
分别注入不存在、空文本、普通文本、UTF-8；原严格负例和完整 runner 保留。
新增拒绝验证防止 Number、Dynamic、nil 和别名 fallback 借错误返回声明成为 String。

WASI 仅验证现有环境能力及可表示的同源契约，不承诺浏览器 core 自动提供环境宿主，
也不因部分通过宣称 Diary 完整 CI 或稳定版本完成。真实 Diary 的完整原 workflow
复验只归因精确的诊断/定义变化；其余 core、模块和应用边界继续分别跟踪。

本地临时目录在验证过程中被外部清理，修复已在仓库目录下重建；删除前候选结果
不充当重建后最终提交的验收。最终测试、CI/review、精确 main 和实际发布包验收
记录在关联 PR 与 issue #1788。
