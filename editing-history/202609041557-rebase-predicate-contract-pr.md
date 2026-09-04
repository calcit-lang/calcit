# Reconcile predicate-contract PR with its updated base / 协调谓词契约 PR 与更新后的基线

## Context / 背景

PR #628 extends the collection-member contract work from PR #624 with filter and iteration callback checks. Updating the stacked base introduced textual conflicts in the shared fixture, assertions, and explanatory documentation.

PR #628 在 PR #624 的集合成员契约工作上增加了 filter 和 iteration callback 检查。更新串联基线后，共享 fixture、断言和说明文档产生了文本冲突。

## Resolution / 解决方案

- Keep the #628 extensions that require `T -> Bool` predicates and preserve Map's heterogeneous pair callback input.
- Inherit the updated FFI safety work and regenerated core-quality metadata through the #624 parent.
- Verify targeted type-fail tests, generated Dynamic classification, and the native quality baseline on the combined revision.

- 保留 #628 对 `T -> Bool` 谓词的扩展，并保持 Map 异构 pair callback 输入。
- 通过 #624 基线继承更新后的 FFI 安全改动和重新生成的 core-quality 元数据。
- 在合并后的 revision 上验证定向 type-fail 测试、生成的 Dynamic 分类和原生 quality baseline。
