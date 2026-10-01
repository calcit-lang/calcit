# WASM 可达占位回退审计（#1533，进行中）

基于未发布 main `0396fe4d366a8b8d39cf80e67821a7b568ac8360`。公共入口直接返回 `quote (+ 1 2)` 时，native 返回引用表达式，实际 WASM 返回 0；`format-to-lisp 42` 的 native 结果为字符串 `42`，WASM 仍成功产出并返回 0。

初步修改让运行时 quote/quasiquote、无法表示的值 import、非静态的 format-to-lisp 返回已有 codegen 错误，不新增诊断分类。保留静态 `(format-to-lisp (quote ...))` 字符串池路径，避免破坏断言错误文本；不把零、Unit 或 metadata 常量统一当成错误。

共同格式化语义通过 CLI 新增到 `calcit.core/format-to-lisp` 的 `:tests`，三个 native 用例通过。已有断言 runner 在初步修改后通过；同一 runner 补充直接 quote、运行时格式化和条件分支的编译拒绝及不产出 artifact 检查后，重新构建与完整专项回放均通过，既有 native/JS 正例、参数/返回负例、async 和标量 WASM 断言保持通过。

第二步复用现有 WASI 的指令调用图检查，从配置入口及实际导出函数追踪直接调用，core WASM 和 Preview 1 同样拒绝可达的编译失败依赖；无关 slot 保留原索引及 trap，不全局激活实现。跨模块 format-value helper 在编译时给出包含具体定义的错误，而同一 helper 未调用时，标量导出仍能实际执行并返回 1。扩展后的完整专项 runner 与既有 WASI 依赖校验单元测试通过。

第三步只记录生成过程中实际发射的函数值引用，把这些具体指向加入同一依赖图，避免全表扫描。把 format-value 保存到局部 callback 再调用的负例现在也在 artifact 生成前被拒绝；完整专项 runner 与 all-target/all-feature Clippy 通过。17a13e66 的完整 Rust 测试通过；函数值补充提交的完整 Rust 仍待确认。

第四步确认跨模块 def 保存的 quote 值走常量内联，在失败信息中补上被导入定义名；Enum 定义值的探针已被既有 `&enum-def:new` unsupported 拦截，不算本次新增修复。参数化 WASM export 的 Bool 两分支分别实际返回合法 0/7，Unit 两分支实际返回现有 ABI 的 0；一般格式化出现在运行时参数分支时仍在编译阶段拒绝。扩展专项回放通过。直接 quasiquote 的 native 正常执行与 WASM 编译拒绝也已确认，并加入同一 runner。

WASI 继续复用 Number 文本 smoke：真实 Component 对运行时 quote/format-to-lisp 编译拒绝且不生成 artifact；静态 quote 格式化的失败断言仍可编译，真实 Wasmtime 执行保留错误文本及非零退出。该扩展 smoke 通过，808d59f7 的完整 Rust 也通过。

现有 core value ABI 的任意宿主表索引由宿主管理，不把本次具体函数值依赖检查宣传为外部索引安全验证；Component 本身拒绝 Fn/closure。文档明确这个边界。剩余全量 check-all、最新提交 Rust/Clippy、独立 review 与 CI 门禁未完成，保持 draft。
