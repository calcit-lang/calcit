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

## Tag

Calcit also provides the Tag type, written with a leading `:` such as `:demo`. Tags are interned immutable identifiers with consistent Calcit semantics across the Rust interpreter and JavaScript output. They are commonly used for struct fields, enum variants, map keys, and protocol labels; ordinary user-facing text should remain a String.
