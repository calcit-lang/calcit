# cr edit schema: support primitive type tag leaves

## 背景

`cr edit schema` 原来只接受 `:: :fn $ {}` 这样的函数 schema 或 `{}` map 形式。
有些定义的类型本身就是简单原始类型（如 `:string`、`:number`），不是函数，需要直接用 tag 表达。

## 新语法

```bash
cr edit schema 'respo.comp.space/style-space' --code 'quote :string'
cr edit schema 'some.ns/my-val' --code 'quote :number'
cr edit schema 'some.ns/my-flag' --code 'quote :bool'
```

接受的 primitive 类型 tag：`bool`, `number`, `string`, `symbol`, `tag`, `list`, `map`, `set`, `fn`, `tuple`, `ref`, `buffer`, `dynamic`, `unit`

## 改动文件

### `src/calcit/type_annotation.rs`

- `builtin_tag_name` 改为 `pub(crate)`，供 snapshot 序列化侧调用。

### `src/snapshot.rs`

- 新增常量 `PRIMITIVE_SCHEMA_TAGS`：列出允许作为 leaf schema 的原始类型。
- `validate_schema_for_write`：对 `Cirru::Leaf` 先检查是否在允许列表，允许则直接 Ok；否则给出提示信息。
- `parse_loaded_schema_annotation`：新增对 `Edn::Tag` 的处理，返回对应 `CalcitTypeAnnotation`。
- 两处 `CodeEntry::From` impl：`_ => Edn::Nil` 改为调用 `builtin_tag_name().map(Edn::tag)` 把原始类型 tag 正确序列化到快照。
- 测试 `test_validate_schema_for_write`：更新为测试原始类型 leaf 通过、未知 leaf 拒绝。

### `src/bin/cli_handlers/edit.rs`

- `handle_schema`：`validate_schema_for_write` 通过后，若 `schema_payload` 是 `Cirru::Leaf`，用 `CalcitTypeAnnotation::from_tag_name` 直接设置 schema 并返回，不走函数 schema 解析路径。

## 完整数据流（以 `:string` 为例）

1. CLI 收到 quoted schema 输入 `--code 'quote :string'`，取出 payload 后得到 `Cirru::Leaf(":string")`
2. `validate_schema_for_write` → `"string"` 在 `PRIMITIVE_SCHEMA_TAGS` → Ok
3. `CalcitTypeAnnotation::from_tag_name("string")` → `CalcitTypeAnnotation::String`
4. 写入快照 → `CodeEntry::From` 序列化为 `Edn::tag("string")`
5. 读回快照 → `parse_loaded_schema_annotation(Edn::Tag("string"))` → `CalcitTypeAnnotation::String`
