# Trait-bound 方法改写后的返回证明

严格升级工作流中，有界泛型函数 `T: Countable` 使用 `.count`，native 原始测试通过，却被返回审计判为 Dynamic。最小复现来自 typed-rest 迁移验证：List Number 的 rest 收集值交给有 Countable 约束的函数，而非无约束的 core `count`。无约束版本仍需要独立的能力契约，不能靠本次修复当作已证明。

原因是方法来源限定改写为 `&trait-call` 后，返回推断只读取 proc 的动态边界签名，丢失已声明的 trait 方法契约。本次从来源明确的源码声明、trait 值或带类型的局部值解析方法，要求接收者具有唯一匹配的名义来源；与表层方法共用 CallTypeProof 的参数证明、泛型替换、where 约束和异步返回包装。读取源码类型不求值 trait 定义，也不按短名称合并来源。

可观察语义保存在独立 fixture 的 definition `:tests`：多个集合的 Countable 方法、自定义 trait 的 String 返回、泛型返回的 Number/String/List 替换以及 typed-rest 转发。既有 assertion 运行器负责严格工作流、原始测试的 native/JS 回放与错误参数/返回声明的拒绝，不增加新检查脚本或迁移规则。Rust 测试只覆盖内部名义来源、重复来源和未证明参数不能借用返回证据。

本次属于 #1694 的真实升级阻塞修复，同时为 #1553 的未来改写后校验保留回归；不代表交付整个校验层，不关闭该 issue。WASM 泛型调用支持仍由 backend 的独立支持边界决定，不借助返回证明静默回退到动态调用。
