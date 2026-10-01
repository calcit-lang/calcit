# quote 的 JS 尾位置返回值

关联 #1609。普通 defn 尾部 quote 在 native 返回引用值，但真实生成 JS 创建值后丢弃，函数返回 undefined；不是 WASM export 特例。Quote 分支漏用了 gen_call_code 已有的 return_label，修复只复用该上下文，不改变引用的构造、类型关系或求值规则。

正例附在 calcit.core/quote 的 :tests，覆盖数字、文本、未绑定符号、引用表达式、let/if 尾位置、集合内的普通表达式上下文，以及引用内容不执行、实参副作用仅执行一次。复用现有返回/断言 runner，在 native 与实际生成 JS 回放；不新增 analyzer、公开入口或独立 Rust 语义测试。

未修复 JS 在数字正例中返回 Unit，与期望 42 不同；修复后全部正例和原有编译拒绝回归通过。最终完整 Rust/Clippy/集成及最新 PR CI 结果以 PR 为准。WASM 运行时 quote 的占位回退属于 #1533，本步不宣称支持该功能，也不依靠修改期望值或给业务添加 let workaround 通过。
