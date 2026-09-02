# F64Buffer length portability / F64Buffer 长度可移植性

## 中文

`F64Buffer` 的 strict length boundary 需要表达“长度必须小于 `2^63`”。此前使用
`1usize << 63`，在 32-bit target 上该常量本身不可表示，可能在用户还未运行程序前就导致编译失败。

比较现在先将 `usize` 长度提升为 `u64`，再与 `1u64 << 63` 比较。语义不变：64-bit target 仍拒绝
无法装入 signed i64 的长度；32-bit target 的所有可表示 `usize` 长度自然低于该上限。这个修复不改变
F64Buffer ABI、Nil/Dynamic policy 或 runtime allocation。

## English

The strict `F64Buffer` length boundary must express that a length is smaller than `2^63`. The previous
`1usize << 63` expression is not representable on a 32-bit target and can fail compilation before a user
runs their program.

The comparison now widens the `usize` length to `u64` and compares it with `1u64 << 63`. Semantics are
unchanged: 64-bit targets still reject lengths that cannot fit in signed i64, while every representable
32-bit `usize` length is naturally below that limit. This does not change the F64Buffer ABI, Nil/Dynamic
policy, or runtime allocation.
