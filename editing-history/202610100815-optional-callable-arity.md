# 可选参数的回调契约

`map ([] 1 2) format-cirru-edn` 在严格检查中错误地报告回调需要两个参数。Proc 元数据已有一到两个参数的运行时范围，但转成函数类型时只保留了参数值类型。`trim` 以及局部 builtin 别名存在同类问题。

函数类型内部保留已有的运行时 arity，泛型替换和命名空间类型解析继续传递这份证据。函数兼容性和严格证明使用同一个原则：实际函数必须接受预期回调承诺的整个调用范围，参数类型仍逆变、返回类型仍协变。只有 schema 的函数从固定、rest 和末尾名义 Option 参数推导范围；带 rest 的固定 Option 参数仍不可省略。

这不增加公开 schema 字段、CLI 命令或诊断编号，不改变参数值类型和 runtime/ABI。builtin 的可选 Bool/String 不变成 nullable，也不伪装成 rest。零参数、超出最大参数数目、错误参数类型及不受支持的 variadic 契约仍然不能通过证明。

用户语义由 definition `:tests` 验证：trim 的共享用例在 native、JS、WASM 和 WASI 上回放；formatter、局部 builtin 别名及末尾 Option 回调在原有 fixture 中用同一运行器回放 native/JS，并接入现有严格断言检查。Rust 只检查内部 Proc 投影、泛型替换和调用范围证明的边界。现有断言、排除规则与严格负例均保留。

WASM 高阶 formatter 当前把元素类型丢成 Dynamic，局部 Proc 值也尚未 lowering；这两个失败独立于前端 callable 契约修复。不为它们增加排除项、改写成手动 native call 或放宽类型，本次不宣称它们已支持。后续应补足类型证据和静态 callable lowering，而不是修改表层规则。
