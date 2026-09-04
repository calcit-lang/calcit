# Close merged-PR review follow-ups

2026-09-05 00:55 CST

## English

- Audited unresolved review threads across the 100 most recently merged PRs and separated still-valid findings from threads already fixed by later commits.
- Revalidated Issues before lease renewal, heartbeat, and takeover; fenced release-side Issue synchronization by repairing the mirror when a newer authoritative lock appears.
- Resolved bound type slots before indexed-access specialization, prevented embedded function hints from exempting macros, recognized the source `%{}?` macro before expansion, and stopped reverse placeholder matching from erasing nominal trait identity.
- Aligned `filter-map-kv`, WASM string-test descriptions, strict migration guidance, backend capability states, and whole-`Dynamic` terminology with their current contracts.

## 中文

- 审计最近 100 个已合并 PR 的未解决 review threads，并区分当前仍成立的问题与已由后续提交修复但未关闭的线程。
- 在租约续期、心跳和过期接管前重新校验 Issue；`release` 完成可见状态同步后若发现更新的权威锁，则自动修复 Issue 镜像。
- indexed access 会先解析已绑定 type slot；宏不会因内部函数 hint 绕过根 schema 检查；strict 模式能在展开前识别 `%{}?`；nominal trait 不再因反向 bare placeholder 匹配而丢失身份约束。
- 同步修正 `filter-map-kv`、WASM 字符串测试说明、strict 迁移文档、backend 能力矩阵和 whole-`Dynamic` 术语。
