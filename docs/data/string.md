---
title: "String"
scope: "core"
kind: "reference"
category: "data"
aliases:
  - "string literals"
  - "pipe prefix"
  - "quoted strings"
---
# String

The way strings are represented in Calcit is a bit unique. Strings are distinguished by a prefix. For example, `|A` represents the string `A`. If the string contains spaces, you need to enclose it in double quotes, such as `"|A B"`, where `|` is the string prefix. Due to the history of the structural editor, `"` is also a string prefix, but it is special: when used inside a string, it must be escaped as `"\"A"`. This is equivalent to `|A` and also to `"|A"`. The outermost double quotes can be omitted when there is no ambiguity.

This somewhat unusual design exists because the structural editor naturally wraps strings in double quotes. When writing with indentation-based syntax, the outermost double quotes can be omitted for convenience.

## Character count and wire size

`count` 返回 Unicode 标量值的数量；协议、队列、文件或指标需要 UTF-8 编码字节数时使用 `&str:utf8-byte-count`。两者在 native、JavaScript 与 WASM 目标上保持一致：

```cirru
assert= 2 $ count |A😀
assert= 5 $ &str:utf8-byte-count |A😀
```

Keep these operations distinct: character count describes Calcit text indexing semantics, while UTF-8 byte count describes storage and wire budgets. The latter is O(1) on the native and WASM representations and uses one allocation-free linear pass in generated JavaScript.

## 索引与切片

native、JavaScript 与 WASM 已验证的静态字符串路径中，`first`、`last`、`nth`/`get`、`rest`、`slice` 和相应方法按 Unicode 标量访问，不能把 JS 的 UTF-16 单元偏移或 WASM 的 UTF-8 字节偏移当成 Calcit 索引。组合字符仍可能包含多个标量；这里不提供字素簇或 locale 分词语义。

```cirru
assert= (%some |😀) $ last |中😀
assert= (%some |中) $ get |A😀中 2
assert= (%none) $ nth |A😀中 3
assert= |😀中 $ .slice |A😀中文 1 3
assert= 2 $ count |é
```

公开读取返回 `Option<String>`；底层 `&str:first` / `&str:nth` 在空串或超范围时返回 nil。`&str:contains?` 判断索引是否存在，不判断子串。底层索引须是非负整数，负数、小数及非有限数会报错；公开 `nth`/`get` 的既有边界 guard 对负数或超范围位置返回 `%none`，不把范围内的小数截断为整数。切片采用左闭右开区间，超出末尾时截到末尾，反向或空区间返回空串。

0.23 的这项 JS 修复无需源码迁移；不要在应用中通过 UTF-16 偏移补偿 emoji 长度。内部使用无字符数组分配的标量扫描，ASCII/BMP 有快速路径。JS FFI 传入的孤立 surrogate 不是合法 Unicode 标量，不在本次跨 backend 保证范围内。

WASM 的底层 `first`/`nth`/`rest`/`slice`/`contains?` 复用有界的标量边界扫描：空串 `rest` 保持空串，超大合法索引按越界处理，复制范围不越过源字符串。普通 `slice` 根据已证明的 String/List receiver 选择现有 lowering，不再无条件解释成列表；缺少类型证据时明确拒绝。传入参数按原顺序各求值一次，再验证索引。

native/JS 的非法索引通过既有错误路径报告；当前 WASM 对应为 trap，尚不提供可捕获的 `try` 语义。共享 Calcit 测试验证正常结果，宿主测试验证 trap 与求值顺序。不将这些结果外推到所有 Dynamic 方法、任意 host 字符串或其他文本操作；本次修复不扩大 WASI 宿主能力。

## Tag

Calcit also provides the Tag type, written with a leading `:` such as `:demo`. Tags are interned immutable identifiers with consistent Calcit semantics across the Rust interpreter and JavaScript output. They are commonly used for struct fields, enum variants, map keys, and protocol labels; ordinary user-facing text should remain a String.
