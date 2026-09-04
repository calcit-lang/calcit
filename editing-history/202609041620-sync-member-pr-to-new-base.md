# Sync member-contract PR to its new base / 同步成员契约 PR 到新基线

## Context / 背景

The base branch for the stacked collection work advanced with a concurrent integration of the member-contract changes. The open member PR must record that new base so GitHub can compute its mergeability from the current graph.

串联集合工作的基线分支通过并发集成推进，其中已包含成员契约改动。打开的成员 PR 必须记录这个新基线，以便 GitHub 从当前图计算其可合并性。

## Change / 改动

- Merge the new base without replacing either collection or lexical FFI fixture coverage.
- Verify the resulting type-fail regression set before updating the PR branch.

- 合入新基线，不替换集合或词法 FFI fixture 覆盖。
- 更新 PR 分支前验证结果 type-fail 回归集。
