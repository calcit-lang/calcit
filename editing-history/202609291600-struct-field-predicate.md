# Struct 字段存在性契约

## 决策

0.28.0 的新 `.contains-field?` 由具名 `ContainsField` trait 提供，公开参数为 `Struct, Tag -> Bool`。不能把方法仅放进旧的 originless Struct 方法袋：严格预处理会拒绝这种没有 trait 来源的方法。实现复用 `&struct:contains?`，但不把 native/JS 中接受 String、Symbol 的历史行为宣称为跨 backend 契约；WASM 使用编译期 Tag ID 对照字段。

保留旧 `.contains?` 与底层 primitive，暂不自动迁移 Struct：当前旧方法同时出现在 `Contains` trait 和 legacy 方法袋中，严格调用有歧义；String/Symbol 字段选择器在 WASM 又无同等证明。后续需先处理来源及兼容边界，再考虑 origin-guarded fix。Enum 位置谓词不在此批。

## 验证

- `calcit.core/contains-field?` 的 definition-attached `:tests` 覆盖 Tag 命中/缺失、Bool 类型和旧 primitive 的 native 兼容输入。
- `test-wasm.main/test-struct-contains-field` 的 `:tests` 与实际 WASM export 验证同一 Tag 命题；生成 JS 也调用同一 export。
- 严格类型检查对 `.contains-field? |x` 报 `W_METHOD_ARG_TYPE_MISMATCH`；不自动将 String 转换为 Tag。
