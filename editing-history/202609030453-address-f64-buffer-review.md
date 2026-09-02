# Address F64Buffer portability review / 处理 F64Buffer 可移植性评审

## 中文

- 修正文档中的“比较现在先将”为“比较时先将”。
- 长度上限直接引用 `i64::MAX`，避免硬编码位移，同时保持“无法装入 signed i64 时拒绝”的边界语义。

## English

- Fix the Chinese wording typo in the original portability note.
- Reference `i64::MAX` directly for the upper bound instead of spelling it as a bit shift, while preserving the same signed-i64 fit rule.
