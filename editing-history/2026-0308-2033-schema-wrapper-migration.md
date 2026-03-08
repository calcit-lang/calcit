# 2026-03-08 schema wrapper migration

## Summary

- 统一函数 schema 的 snapshot 输出格式，顶层改为 wrapped `:: :fn` / `:: :macro` 形式。
- 修复 schema round-trip 中的类型信息丢失，确保嵌套函数类型与 `:rest` 在格式化后保持稳定。
- 将 macro schema 的顶层格式统一迁移到 `:: :macro`，并让 formatter 在处理 `defmacro` 条目时自动纠正旧的 `fn` 标记。
- 移除 legacy quote-wrapped schema normalization 与 legacy positional `fn` type annotation fallback，收紧到当前 schema-map 形式。
- 更新 `deftrait` 文档与相关说明，避免继续传播旧写法。

## Main changes

### 1) 顶层 schema 输出统一

- 在 `src/calcit/type_annotation.rs` 中补充顶层 wrapped schema 的解析与序列化逻辑。
- 在 `src/snapshot.rs` 中让 `CodeEntry.schema` 写回 snapshot 时统一输出为 `:schema $ :: :fn ...` 或 `:schema $ :: :macro ...`。
- 对 plain fn schema 去掉 wrapped payload 内冗余的 `:kind :fn`，但保留宏函数所需的 `:kind :macro`。

### 2) macro schema 迁移与规范化

- 扩展 parser / normalize 逻辑，使其同时接受 wrapped `fn` 与 wrapped `macro`。
- 对 macro schema，序列化时省略冗余的 `:kind :macro` 与 `:return`，仅保留 `:args` 与可选 `:rest`。
- 在 snapshot 读写路径中，根据代码头是否为 `defmacro` 自动纠正 schema kind。

### 3) legacy fn syntax 收紧

- 移除 legacy quote-wrapped schema normalization。
- 拒绝 legacy quoted generic symbols。
- 移除 legacy positional `fn` type annotation parsing fallback。
- 迁移剩余 core trait 与测试注解到 schema-map `:: :fn {}` 形式。

### 4) round-trip 与批量格式化

- 修复 EDN 到 Calcit 的转换路径，补上 `Edn::Map` 的处理，避免嵌套 `fn` schema 的 hashmap payload 被吞掉后退化成 `:dynamic`。
- 调整 schema 校验与 normalize 逻辑，使 wrapped 顶层 fn schema 与旧格式都能稳定读写。
- 重新格式化了 `src/cirru/calcit-core.cirru`、`calcit/util.cirru`、`calcit/test-hygienic.cirru` 等兼容 snapshot 文件。

## Validation

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `yarn compile`
- `cargo test -q`
- `cargo build -q`
- `yarn check-all`
