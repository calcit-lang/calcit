# JS emitter 统一空 Map 的内部表示

## 背景

严格类型预处理之后，空 Map `{}` 既可能表现为 `Calcit::Proc(NativeMap)`，也可能保留为指向 core 的 `Calcit::Import`。此前 JS emitter 只识别前一种表示，后一种会被输出为不存在的 `$clt._$M_`。

## 修改

- 新增统一的 native Map 引用判定，同时覆盖 proc 与 core import。
- 空 Map 作为值时调用运行时构造器。
- Map literal 处于调用位置时保留构造器本身，再传入键值参数。
- 为两种内部表示补充 Rust 回归测试。

## 验证

- 运行相关 Rust 单元测试与完整测试、格式和 Clippy 检查。
- 使用修复后的编译器重新生成真实严格类型项目的 JS，并运行前端生产构建，确认不再出现缺失 `$clt._$M_` 导出的警告。
