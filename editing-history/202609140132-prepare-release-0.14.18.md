# 准备 Calcit 0.14.18 / Prepare Calcit 0.14.18

## 中文

- 回顾 milestone 22 与 `0.14.17..main` 的全部变更，确认诊断驱动的 `fix`、显式 syntax-node 输入、Markdown human 输出、声明式验证 Profile、Cirru EDN-first 结构化输出与 mutation guard 已完整合入。
- 将升级、CLI 与工作流文档中的新 `fix` 示例统一为 Cirru EDN-first；JSON 继续作为 JSON-only consumer 的显式互操作格式。
- 将升级指南的发布组合更新为 Calcit 0.14.18 与独立的 `calcit-caps` 0.1.1。
- 修正 crates.io package metadata 中仍指向历史仓库名 `calcit.rs` 的 URL。
- 为 `@calcit/procs` 增加发布文件 allowlist 与 MIT SPDX metadata，只携带编译后的 `lib/` 与 README，不再把 Rust 源码、RFC 和历史记录放入 npm tarball。
- 保留 Calcit 0.14.15 的一次性 migration bridge 说明，以及内部 `cr-wasm` 兼容 wrapper；二者仍分别服务旧项目迁移和仓库发布验证，不属于本次可删除内容。
- 根据 review 同步升级 Cargo、npm 与 lockfile 版本，并补全 fix envelope 中位于 `:data` 下的字段路径。

## English

- Reviewed milestone 22 and every change in `0.14.17..main`, confirming that diagnostic-driven fixes, explicit syntax-node input, Markdown human output, declarative verification profiles, EDN-first structured output, and mutation guards are integrated.
- Made the new fix examples in upgrade, CLI, and workflow documentation EDN-first while retaining JSON as an explicit interoperability format for JSON-only consumers.
- Updated the documented release pairing to Calcit 0.14.18 and the standalone `calcit-caps` 0.1.1.
- Corrected the crates.io package metadata URL that still referenced the historical `calcit.rs` repository name.
- Added an `@calcit/procs` publication allowlist and MIT SPDX metadata so the npm tarball contains only the compiled `lib/` tree and README instead of Rust sources, RFCs, and editing history.
- Retained the one-way Calcit 0.14.15 migration bridge documentation and the internal `cr-wasm` compatibility wrapper because they still serve old-project migration and repository release verification respectively.
- Following review, synchronized the Cargo, npm, and lockfile versions and corrected fix-envelope field paths nested beneath `:data`.
