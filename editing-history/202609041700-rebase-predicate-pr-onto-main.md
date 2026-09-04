# Merge predicate-contract PR onto main / 将谓词契约 PR 合入 main

## Context / 背景

PR #628 was retargeted to `main` after the base branch merged the collection-member contract work. Its branch already contained the equivalent member changes plus predicate and map extensions, so the overlap required an explicit merge record.

集合成员契约工作合入基线后，PR #628 被改为以 `main` 为目标。该分支已包含等价的成员改动以及 predicate 和 map 扩展，因此重叠需要显式的 merge 记录。

## Resolution / 解决方案

- Retain the #628 collection predicate/map extensions and lexical FFI fixture coverage.
- Merge current `main` as a parent without duplicating the already integrated member-contract implementation.
- Verify format, the type-fail subset, generated Dynamic classification, and the core quality baseline.

- 保留 #628 的集合 predicate/map 扩展与词法 FFI fixture 覆盖。
- 将当前 `main` 作为 parent 合入，不重复已集成的成员契约实现。
- 验证格式、type-fail 子集、生成的 Dynamic 分类和 core quality baseline。
