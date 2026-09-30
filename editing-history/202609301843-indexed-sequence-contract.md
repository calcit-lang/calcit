# 索引访问的共享类型证据

0.28.0 的 #1458 要求 Agent 查询不要把编译器已经能静态处理的方法仍标成开放。main `78a1d0b` 的 `query type 'String` 对 `.first/.last/.nth` 显示 `open`，因为这些公开 wrapper 的泛用 schema 返回 `Option<Dynamic>`；预处理却已经按已知 List/String/Enum 接收者分别 lowering，返回类型又在另一处单独推断。

本次把 List<T> 和 String 的 `first/last/nth` 参数、`Option` payload 与静态 lowering 选择汇入已有 `CheckedCallContract`。Enum 维持 `Option<Dynamic>`，不把异质 payload 说成具体类型；Dynamic 接收者也不获得静态证明。query、调用检查、返回推断和 lowering 因此读取同一结果，原来重复的序列 payload 推断分支被删除。用户语义与求值顺序不变，不增加新的公开入口或类型检测规则。

Calcit definition `:tests` 检查 List/字符串成功值与 Option payload，Rust 测试只钉住内部共享契约和 Agent 查询 envelope。验证范围包括 Native、实际生成 JS，以及已有 WASM 子集；未被静态识别的接收者仍保留原有运行时路径。

# 关于 #1456 的 turn-string 试验

给底层 `turn-string` schema 单独增加 `T: ToString` 不足以让 proc 的 Dynamic 参数被静态检查；即使通用 proc 检查读取此 bound，用户自定义 Struct 可实现 ToString，而原始 primitive 仍拒绝 Struct，因而这种声明不健全。试验已撤回。后续须区分可扩展的公开 `to-string` 与仅接受内建标量的底层 primitive，不能用开放 trait 充当封闭标量联合类型。
