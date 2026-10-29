# Struct 字段默认值

#2479 定了按 Rust 的思路用具名 Struct 表达记录形状的数据之后，“总是有值、但多数调用者用同一个值”的字段只能每处都写，或者声明成 `Option` 再在每个读取点 `.unwrap-or` 常量。#2489 按 Rust 的字段默认值（RFC 3681）加了声明处默认值：`defstruct Cfg (:host 'String) (:port 'Number 8080)`。

默认值只接受字面量（数字、字符串、tag、布尔值、空的 `[]` `{}` `#{}`）。这样默认值与命名空间无关，可以原样放到任何调用点，没有求值顺序和副作用问题。`defstruct` 把第三项 quote 后交给 `&struct-def:new`，所以运行时定义和静态解析（`parse_defstruct_code`）读到的是同一个源码形式，`(+ 1 2)` 这类表达式在两边都不会被当作默认值。定义处检查默认值与字段类型是否相符；`Option` 字段固定以 `Option :none` 为默认值，泛型参数字段不允许默认值。

补齐只发生在 preprocess：直接构造器和 map 字面量/`?{}` 改写成 Struct 时，省略字段填入默认值字面量（空容器生成已解析的构造调用）。生成的构造调用和手写全部字段相同，所以 native、JS、WASM 的值表示和 codegen 都不变；`CalcitStructDef.field_defaults` 只供编译期读取，同时参与定义相等性、hash 和排序。低层 `%{}` 不补默认值。

`edit format` 的类型规范化只处理 defstruct 字段的第二项，避免把 `:read` 这类 tag 默认值当作旧式类型改写。

验证：`cargo test --lib struct_field_defaults` 覆盖定义处的接受与拒绝；`calcit/test-struct.cirru` 的 `DefaultFields`、`default-fields-from-literal` `:tests` 覆盖构造器、零参数构造与字面量改写，并由 `run-core-tests.mjs` 在 native 与 JS 上回放。
