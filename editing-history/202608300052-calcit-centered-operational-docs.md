# Calcit-centered operational documentation

## Context / 背景

The primary documentation was already repositioned around Calcit's nominal
types, traits/methods, typed boundaries, and Calcium Workflow. A few syntax and
agent guides still explained ordinary Calcit rules through repeated external
language comparisons.

入口文档已经围绕 Calcit 的 nominal types、traits/methods、typed boundaries 与
Calcium Workflow 重整，但部分语法及 Agent 指南仍通过反复对比其他语言来解释普通规则。

## Change / 修改

- Describe collection constructors, schemas, and collection-first argument
  order directly as Calcit semantics.
- Replace the broad migration-comparison section in the advanced Agent guide
  with Calcit semantic checkpoints based on CLI evidence.
- Keep historical language references only in the dedicated migration page,
  standard provenance notes, and historical source links.

- 直接以 Calcit 语义说明集合构造器、schema 与集合优先的参数顺序。
- 将高级 Agent 指南中的大段迁移对照改为基于 CLI 证据的 Calcit 语义检查点。
- 仅在专门迁移页、标准来源说明和历史资料链接中保留外部语言引用。

## Verification / 验证

`bash scripts/check-docs-md.sh` passes all 60 documentation files and all
325 checked Cirru blocks.

`bash scripts/check-docs-md.sh` 已通过全部 60 个文档文件及 325 个 Cirru 代码块。
