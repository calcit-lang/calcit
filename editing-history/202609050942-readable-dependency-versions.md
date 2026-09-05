# Keep dependency versions readable / 保持依赖版本可读

## 中文

- 在全局 Agent 规则中明确：正式依赖、CLI 与 GitHub Actions 使用精确 release 版本/tag 作为版本 source of truth。
- 禁止用裸 commit hash 或附带版本注释的 hash 代理版本号；精确 release tag 优先于可移动的 major tag。
- 保留少量明确例外：未发布源码验证、问题复现/二分，或已有供应链策略要求时可以固定完整 hash，但必须记录原因及对应版本，且只把它视作 revision/完整性证据。

## English

- Make exact released versions or tags the source of truth for dependencies, CLIs, and GitHub Actions.
- Do not use a raw commit hash, even with a version comment, as a proxy version number; prefer an exact release tag over a moving major tag.
- Allow full commit pins only for documented unreleased-source work, reproduction/bisection, or an explicit immutable-revision policy, with the canonical release version recorded separately.
