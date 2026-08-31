# 2026-08-31 17:21 Fix caps documentation category

## 中文

- 将 `caps-extraction-contract.md` 的 frontmatter category 从未注册的 `packages` 改为已注册的 `run`，使
  `calcit docs list/read/search` 可以加载完整 guidebook。
- 增加直接读取仓库文档并调用 frontmatter validator 的回归测试，防止 extraction contract 再次绕过
  docs category 契约。

## English

- Changed the `caps-extraction-contract.md` frontmatter category from unregistered `packages` to registered `run`,
  allowing `calcit docs list/read/search` to load the complete guidebook.
- Added a regression that reads the repository document and validates its frontmatter so the extraction contract
  cannot drift outside the docs category contract again.
