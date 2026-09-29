# WASM 字符串转换的静态语义边界

`turn-string` 在 native 中按值类型处理 Bool、Tag、Number、Nil 和 String。WASM 当前将标量统一编码为 `f64`，其中 `false`、`nil` 与数字零同为 `0.0`，Tag 则是内部整数 ID；因此仅凭运行时数字无法恢复表层类型。

本次利用预处理后的静态类型证据分别 lower Bool 与 Tag，避免把布尔值或 Tag ID 当成 Number 输出。Number 字面量在编译期使用与 native 相同的 Rust 数字格式化，并把结果放入字符串池。对含局部变量的 Bool/Tag 路径新增 definition-attached `:tests`，同时在 Node 中实际执行生成的 WASM。

这不是完整的数字转换实现：现有 `__rt_f64_to_str` 对运行时非整数仍使用占位结果，动态类型也不能从 `f64` 恢复值种类。后续需要独立的 WASM 浮点最短往返格式化方案和动态值表示边界；在这些得到跨目标验证前，#1456 保持开放，不宣称 `turn-string` 在 WASM 已全面一致。
