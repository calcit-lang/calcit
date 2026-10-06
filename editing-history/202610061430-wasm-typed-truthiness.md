# WASM 条件按类型保留 Calcit 真假值

## 问题与共同语义

JS 条件已经按 Calcit 合同修正，但 WASM 仍对每个 f64 条件比较非零，因此 Number 0、负零和第一个函数表槽位会被误当假值。语言合同只有 nil、false、Unit 为假；目标表示不能改变这一合同。

## 选择

复用现有类型推断与静态调用特化。已证明的 Number、String、集合、名义值和函数条件生成固定真假值；Bool 与表示无歧义的 Optional 保留运行时非零比较。固定真假值只替换判断，不消除条件表达式：先求值一次并丢弃载荷，再执行选中的分支。现有预处理的必然退出证明提供 Never，不复制 raise 名称特判或执行条件来猜测类型。

开放或可空数字条件没有足够的标量表示证据时，复用 E_WASM_NIL_TYPE_EVIDENCE 拒绝产物，不引入动态 runtime tag、表层类型限制或新的诊断编号。已知静态调用可以在原有特化路径补充实际参数类型；不能证明的公开输入仍明确 unsupported。

unsafe-coerce 在 native/JS 保留原值；WASM 不能把 Number 0 当 Bool false，也不能通过局部别名或返回 Bool 的生产者借来虚假证据。保持相同类型、已有单向类型关系证明的静态提升及擦除到 Dynamic 可以继续传递相同表示，未证明的重新标注明确拒绝。review 中的 UInt32 到 Number 擦除沿用 is_proven_for，不增加每种 refinement 的例外表；Number 到 UInt32 仍需要边界证明。允许擦除也保留 assert= 宏的既有实现，不放宽后续对 Dynamic 的具体使用。

同源副作用回放暴露了已有的全局索引错误：atom 从槽位 0 开始，而模块槽位 0 是 i32 heap pointer；reset! 因而生成类型错误的模块。atom 映射修正为从槽位 1 开始，与既有模块布局一致，使用同一 Calcit 副作用合同验证，不通过 JS 宿主计数代替语言表达式。

## 验证安排与边界

新增语义放在 core if 的 definition :tests：数字、非有限数字、可空 Bool/String、函数和定义值；副作用次数使用 test-struct 的附带测试。既有 check-known-assertion runner 在 native/JS 回放全部同源 AST，并在 WASM 回放支持的完整组与全局状态副作用合同。review 补充从原有异常传播测试提取三个完整 if 表达式，分别验证条件、选中 then/else 分支的 raise 在真实 WebAssembly 中产生 RuntimeError；只移除外层 try 捕获，不改表达式或声称支持 WASM try。脚本专属检查限于真实导出的 f64 参数、零函数槽位、空字符串地址和拒绝产物的 ABI 边界；原有严格正反例继续完整执行。

这不提供完整 Dynamic ABI、任意闭包或所有异常捕获能力，不改变 native/JS 语义，不代表新版本已正式发布。PR 最新 HEAD 的 review/CI、合并后的精确 main workflow 与正式包验收仍是交付门禁。
