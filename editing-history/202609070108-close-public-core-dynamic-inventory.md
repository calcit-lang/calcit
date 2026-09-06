# Close the remaining public-core Dynamic inventory

Issue: calcit-lang/calcit#701

## 中文

- 逐个复核剩余 28 个 `migrate` 位置，并按真实实现证据分成 compiler-specialized contract、显式兼容边界、runtime internal 与 test-only non-returning boundary。
- List/Set/Map HOF、索引访问、`update` 以及 Map-only literal path 已由预处理阶段恢复输入/输出关系，并有正向 lowering/inference 与 callback/type-fail 回归；保留 facade 中的 Dynamic 不再冒充静态证据。
- Cirru 递归异构树、通用字符串化、动态路径谓词、运行时 shape inspection，以及缺少 `Never` 类型时的测试抛错返回，均以精确 schema path 和理由冻结。
- 分类器仍把未来未识别的 caller-visible Dynamic 默认放入 `migrate`，因此本次清零不会允许后续债务静默增长。
- 为精确位置分类函数补充说明，使保守 fallback 对生成表维护者和审查工具都保持可见。

## English

- Audited all 28 remaining `migrate` positions and classified each from implementation evidence as a compiler-specialized contract, explicit compatibility boundary, runtime internal, or test-only non-returning boundary.
- List/Set/Map HOFs, indexed access, `update`, and Map-only literal paths already recover their input/output relations during preprocessing with positive lowering/inference and callback/type-fail regressions; Dynamic in their public facades is no longer presented as static evidence.
- Recursive heterogeneous Cirru trees, universal string presentation, dynamic-path predicates, runtime shape inspection, and the test failure return in the absence of a `Never` type are frozen with exact schema paths and rationales.
- The classifier still assigns future unknown caller-visible Dynamic positions to `migrate`, so reaching zero here does not permit later debt to grow silently.
- Documented the exact-position classifier itself so its conservative fallback is visible to generated-report maintainers and review tooling.
