# Inherit FFI fixture coverage in predicate-contract PR / 在谓词契约 PR 中继承 FFI fixture 覆盖

## Context / 背景

The predicate-contract branch is stacked on the member-contract branch. The parent received an additive correction that restores independent lexical FFI tests and their fixture documentation.

谓词契约分支基于成员契约分支。父分支收到一个加法式修复，恢复了独立的词法 FFI 测试及其 fixture 文档。

## Resolution / 解决方案

- Merge the parent correction without changing the predicate/iteration contract extensions.
- Keep the combined type-fail coverage available to the top PR.

- 合入父分支修复，不改变 predicate/iteration 契约扩展。
- 让顶层 PR 也保留组合后的 type-fail 覆盖。
