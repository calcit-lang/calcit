# Preserve FFI fixtures while resolving the PR stack / 在解决 PR 串联时保留 FFI fixtures

## Context / 背景

The member-contract conflict shared `type_fail.rs` with newly added lexical FFI checks. The member-contract version was the correct resolution for the overlapping collection test, but the independent strict FFI fixture assertions and README inventory must remain present.

成员契约冲突与新加入的词法 FFI 检查共用 `type_fail.rs`。成员契约版本适用于重叠的集合测试，但独立的 strict FFI fixture 断言和 README 清单也必须保留。

## Resolution / 解决方案

- Retain both the collection-member test and the four strict FFI test cases.
- Keep the README commands, diagnostics, and test descriptions for the FFI fixtures.
- Run the complete type-fail test subset after combining the independent changes.

- 同时保留集合成员测试和四个 strict FFI 测试用例。
- 保留 README 中针对 FFI fixture 的命令、diagnostic 和测试说明。
- 合并独立改动后运行完整 type-fail 测试子集。
