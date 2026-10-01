# WASM 可达占位回退审计（#1533，进行中）

基于未发布 main `0396fe4d366a8b8d39cf80e67821a7b568ac8360`。公共入口直接返回 `quote (+ 1 2)` 时，native 返回引用表达式，实际 WASM 返回 0；`format-to-lisp 42` 的 native 结果为字符串 `42`，WASM 仍成功产出并返回 0。

初步修改让运行时 quote/quasiquote、无法表示的值 import、非静态的 format-to-lisp 返回已有 codegen 错误，不新增诊断分类。保留静态 `(format-to-lisp (quote ...))` 字符串池路径，避免破坏断言错误文本；不把零、Unit 或 metadata 常量统一当成错误。

共同格式化语义通过 CLI 新增到 `calcit.core/format-to-lisp` 的 `:tests`，三个 native 用例通过。已有断言 runner 在初步修改后通过；同一 runner 补充直接 quote、运行时格式化和条件分支的编译拒绝及不产出 artifact 检查后，重新构建与完整专项回放均通过，既有 native/JS 正例、参数/返回负例、async 和标量 WASM 断言保持通过。

第二步复用现有 WASI 的指令调用图检查，从配置入口及实际导出函数追踪直接调用，core WASM 和 Preview 1 同样拒绝可达的编译失败依赖；无关 slot 保留原索引及 trap，不全局激活实现。跨模块 format-value helper 在编译时给出包含具体定义的错误，而同一 helper 未调用时，标量导出仍能实际执行并返回 1。扩展后的完整专项 runner 与既有 WASI 依赖校验单元测试通过。

第三步只记录生成过程中实际发射的函数值引用，把这些具体指向加入同一依赖图，避免全表扫描。把 format-value 保存到局部 callback 再调用的负例现在也在 artifact 生成前被拒绝；完整专项 runner 与 all-target/all-feature Clippy 通过。17a13e66 的完整 Rust 测试通过；函数值补充提交的完整 Rust 仍待确认。

尚未完成：值 import 公共路径的可达性分类、宿主提供函数值与运行时参数的边界、错误分支、WASI 验证、source 定位精度、文档与完整门禁。没有把所有间接调用统一拒绝，也没有声称任意宿主传入的表索引已验证安全；需保留这些边界的明确验收证据。
