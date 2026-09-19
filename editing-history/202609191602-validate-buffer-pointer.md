# 验证 Buffer 指针整数性 / Validate Buffer pointer integrality

## 中文

- 在 WASM Buffer 字面量比较读取堆头以前，同时验证候选值位于堆范围且是整数，避免带小数的 Number 被截断后误认成相邻 Buffer 指针。

## English

- Before WASM Buffer-literal equality reads a heap header, require the candidate to be both in heap range and integer-valued so a fractional Number cannot truncate to an adjacent Buffer pointer.
