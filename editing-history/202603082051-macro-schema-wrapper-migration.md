# 本次修改记录

## 主题

- 将 macro schema 的顶层格式统一迁移到 `:: :macro`。
- 让 formatter 在处理 `defmacro` 条目时自动纠正旧的 `fn` 标记。
- 收紧 macro schema 输出，只保留参数信息与 `:rest`，省略固定的返回信息。

## 主要改动

### 1) parser / normalize 兼容 `:: :macro`

- 在 `src/calcit/type_annotation.rs` 中扩展 schema 解析逻辑，使其同时接受 wrapped `fn` 与 wrapped `macro`。
- 在 `src/snapshot.rs` 中扩展 normalize 与 write validation，允许 `(:: :macro ({} ...))` 作为合法顶层 schema 形式。

### 2) formatter 输出新 macro 格式

- `to_wrapped_schema_edn()` 现在会根据 `fn_kind` 输出 `:: :fn` 或 `:: :macro`。
- 对 macro schema，序列化时省略冗余的 `:kind :macro` 与 `:return`，仅保留 `:args` 与可选 `:rest`。
- 嵌套函数类型仍维持原有 `:: :fn` 表示，不影响普通函数 schema 输出。

### 3) 通过 format 自动迁移旧 snapshot

- 在 snapshot 读写路径中，根据代码头是否为 `defmacro` 自动纠正 schema kind，保证历史上被写成 `fn` 的 macro 定义在重新 format 后迁移到 `:: :macro`。
- 重新格式化了 `src/cirru/calcit-core.cirru`、`calcit/util.cirru`、`calcit/test-hygienic.cirru`，将已有 macro schema 迁移到新格式。

## 验证

- `cargo fmt` 通过。
- `cargo build -q` 通过。
- `cargo test -q` 通过。
- `yarn check-all` 通过。
