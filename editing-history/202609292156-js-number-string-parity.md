# Number 文本的 native/JS 契约

`turn-string` 在 native 使用 Rust `f64` 的 Display 文本：有限值采用展开的十进制写法，并保留 `-0`；非有限值分别为 `inf`、`-inf`、`NaN`。JS 原先直接调用 `Number.toString()`，在指数边界输出 `1e-7`、`1e+21`，且把 `-0` 变为 `0`，造成相同 Calcit 表层值的跨后端文本差异。

本次只修改 JS runtime 的数字分支：将 JavaScript 最短有效数字中的指数位置展开为普通十进制字符串，并明确处理负零和非有限值。Calcit core definition `:tests` 验证 native 语义；JS 回归脚本对同一表达式实际执行 native CLI，再比较 JS runtime。其他值类型与转换输入范围不变。

WASM 上运行时计算的小数仍由独立的格式化问题阻塞；这次不会借 JS 修复宣称三后端全面一致，也不更改 `turn-string` 的过宽泛型 schema。#1456 继续跟踪类型契约及消费者迁移。
