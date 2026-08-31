# 收尾近期 FFI review / Close recent FFI review findings

- WIT 生成器按规范区分 `Result<T, Unit>`、`Result<Unit, E>` 与 `Result<Unit, Unit>`，分别输出省略空 payload 的 canonical forms。
- 在渲染前按 Rust/TypeScript、Calcit/WIT 与 native symbol 各自的规范化规则检查生成标识符冲突，并覆盖跨命名空间同名定义。
- native FFI base symbol 明确拒绝三个已发布 ABI 的协议后缀，避免运行时再次追加后查找不存在的入口。
- 功能 PR 不单独修改 manifest 版本；版本仍由合并后的 release 提交统一同步。

- The WIT generator now distinguishes `Result<T, Unit>`, `Result<Unit, E>`, and `Result<Unit, Unit>`, emitting canonical forms with absent payloads omitted.
- Generated identifiers are checked before rendering under the normalization rules for Rust/TypeScript, Calcit/WIT, and native symbols, including same-name definitions from different namespaces.
- Native FFI base symbols explicitly reject all three published ABI protocol suffixes, preventing runtime lookup after a second suffix is appended.
- Feature PRs do not bump manifest versions independently; synchronized versions remain the responsibility of the post-merge release commit.
