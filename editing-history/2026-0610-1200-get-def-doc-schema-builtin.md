# &get-def-doc / &get-def-schema

- 新增 `&get-def-doc`、`&get-def-schema` 内置函数，参数为 `ns/def` 字符串或 symbol。
- Rust/eval 运行时通过 `program::lookup_def_doc` / `lookup_def_schema` 读取已加载定义的 `:doc` 与 `:schema`。
- `snapshot::schema_annotation_to_edn` 统一 schema 转 EDN。
- calcit-js 标记为 unavailable，不生成 def-meta 注册表，避免 js-out 膨胀。
- 测试：`calcit/test-def-meta.cirru`，仅在 `inside-eval:` 下运行。
