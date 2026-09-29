# WASM Number 文本格式化的特殊值

关联 [#1516](https://github.com/calcit-lang/calcit/issues/1516)。运行时 `Number` 的 `turn-string` 以前只接受安全整数；负零、NaN 和正负无穷大即使在 native 与 JS 已有一致的文本表示，WASM 仍会 trap。本次在现有 `__rt_f64_to_str` 中直接按 native 拼写生成 `-0`、`NaN`、`inf` 和 `-inf`，不增加宿主导入，也不引入第二个转换入口。core WASM 与 WASI 共用这段运行时。

回归脚本现在直接读取 WASM 返回的 UTF-8 字节，覆盖这些特殊值和已有的安全整数，并补充 native/JS 的有限数字边界矩阵。有限小数、超出安全整数范围的运行时整数与指数边界仍会明确 trap；它们需要后续确定性的最短十进制算法。本改动不是 #1516 的完整验收，也不应据此关闭 issue。

进一步对 100 万个随机有限 `f64` 比较后，发现 Rust `Display` 与 JS/Ryū 在 252 个“两个最短表示都能回读到同一浮点数”的末位选择上不同，例如 `864310392341871.3` 与 `864310392341871.2`。为避免 native、JS 和后续 WASM 永久维持两种舍入规则，语言级 `turn-string`、`str` 及 WASM 字面量预格式化统一采用 Ryū 的最短数字，再展开指数为普通十进制。此处是极少数文本输出的可观察语义变更，不改变 Number 的二进制值或解析规则；关联 PR 在完整 WASM 运行时验收前保持 draft。
