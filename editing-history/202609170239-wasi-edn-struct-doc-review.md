# 明确 WASI Struct Cirru EDN 的标量边界

## 中文

- 明确列出顶层 nominal Struct 当前可以完成 parse/format 往返的标量字段类型。
- 说明 typed parser 虽可读取 `Number`、`Float32` 与 `Float64`，formatter 尚不支持这些运行时浮点类型，因此不承诺往返。

## English

- Enumerate the scalar field types that currently complete top-level nominal Struct parse/format round trips.
- Clarify that the typed parser accepts `Number`, `Float32`, and `Float64`, while the formatter does not yet support those runtime floating types and therefore does not promise a round trip.
