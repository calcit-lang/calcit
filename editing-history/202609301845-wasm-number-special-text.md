# WASM Number 文本格式化的特殊值

关联 [#1516](https://github.com/calcit-lang/calcit/issues/1516)。运行时 `Number` 的 `turn-string` 以前只接受安全整数；负零、NaN 和正负无穷大即使在 native 与 JS 已有一致的文本表示，WASM 仍会 trap。本次在现有 `__rt_f64_to_str` 中直接按 native 拼写生成 `-0`、`NaN`、`inf` 和 `-inf`，不增加宿主导入，也不引入第二个转换入口。core WASM 与 WASI 共用这段运行时。

回归脚本直接读取 WASM 返回的 UTF-8 字节，覆盖特殊值、安全整数、有限小数、超出安全整数范围的整数和指数边界，并与 native/JS 对照。运行时经固定版本的 Ryū 辅助模块生成最短数字并展开指数；辅助源码、锁文件、再生脚本和生成的 WASM object 一同提交，编译产物内嵌函数与只读表，无宿主格式化导入。辅助栈、400 字节输出缓冲区、只读表与 Calcit 堆分区放置；WASI 与 core WASM 共用同一实现。

进一步对 100 万个随机有限 `f64` 比较后，发现 Rust `Display` 与 JS/Ryū 在 252 个“两个最短表示都能回读到同一浮点数”的末位选择上不同，例如 `864310392341871.3` 与 `864310392341871.2`。为避免 native、JS 和后续 WASM 永久维持两种舍入规则，语言级 `turn-string`、`str` 及 WASM 字面量预格式化统一采用 Ryū 的最短数字，再展开指数为普通十进制。此处是极少数文本输出的可观察语义变更，不改变 Number 的二进制值或解析规则；关联 PR 在完整 WASM 运行时验收前保持 draft。
