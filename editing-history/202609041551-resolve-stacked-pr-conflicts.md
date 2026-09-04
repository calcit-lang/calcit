# Resolve stacked collection-contract PR conflicts / 解决串联集合契约 PR 冲突

## Context / 背景

PR #622 is the base of the stacked #624 and #628 changes. Its branch diverged from `main` after the lexical FFI safety work updated generated core-quality inventory metadata.

PR #622 是串联的 #624 和 #628 改动的基线分支。词法 FFI 安全改动更新了自动生成的 core-quality 清单元数据后，该分支与 `main` 发生分歧。

## Resolution / 解决方案

- Merge the current `main` into the #622 branch.
- Regenerate the per-definition quality baseline and Dynamic classification from the merged core source rather than selecting either stale generated count.
- Verify the targeted type-fail suite plus the quality and generated-document checks.

- 将当前 `main` 合入 #622 分支。
- 从合并后的 core 源码重新生成 definition 级质量 baseline 和 Dynamic 分类，避免选用任一过期的生成计数。
- 验证定向 type-fail 测试，以及质量和生成文档检查。
