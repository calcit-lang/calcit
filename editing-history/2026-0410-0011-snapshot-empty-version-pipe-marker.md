# 2026-0410-0011 — 修复 snapshot `configs.version` 对 `||` 空字符串的漏判

## 背景

PR `snapshot-format` 新增了空 `configs.version` 的回归测试，但 GitHub Actions 上加载 `compact.cirru` 时，`(:version ||)` 会解析成字符串 `"|"`，没有命中原先仅检查空白字符串的校验。

## 知识点

- Cirru 的空字符串字面量 `||` 在这条解析路径里会落成 `"|"`，不能只靠 `trim().is_empty()` 判断是否为空。
- `configs.version` 的校验要同时覆盖真正空串和这个 pipe marker，才能在加载阶段稳定报出 `configs.version cannot be empty`。

## 修改

- `src/snapshot.rs`
  - 将 `parse_snapshot_config_string_field` 中 `version` 的空值判断扩展为：`text.trim().is_empty() || text == "|"`。

## 验证

```bash
cargo fmt -- src/snapshot.rs
```

本地 `cargo test` 在当前机器仍受 macOS 链接器环境影响（`ld: library 'System' not found`），实际正确性改由 GitHub Actions 复核。