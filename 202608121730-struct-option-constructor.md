# Struct 头部构造与 Option 缺省字段

- Struct 定义现在统一支持 `Struct :field value` 的类型化头部调用语法。
- 构造器继续要求所有非 Option 字段显式提供，并校验字段名称、重复字段和字段值类型。
- 名义 `Option<T>` 字段可以省略，预处理会补成 `%none`；旧 `Optional<T>` 仍保持 `nil` 兼容语义。
- 补充 Struct、CalcitAgent 和类型指导文档，并新增预处理回归测试。
