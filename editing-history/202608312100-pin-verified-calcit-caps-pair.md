# 固定升级文档中的 Calcit/caps 验证组合 / Pin the verified Calcit/caps pair in upgrade docs

## 中文

- 根据 PR review 修正 `docs/run/upgrade.md` 中两组无版本 `cargo install` 示例。
- 当前明确固定已完成本地与 `respo-calcit-workflow` smoke 的 `calcit 0.13.72` + `calcit-caps 0.1.0`。
- 后续升级必须从 release notes 或 `setup-calcit` 已验证矩阵选择明确组合，避免两个独立发布工具各自隐式取 latest。

## English

- Fixed both unversioned `cargo install` examples in `docs/run/upgrade.md` after PR review.
- The commands now pin the locally and `respo-calcit-workflow`-verified pair: `calcit 0.13.72` plus `calcit-caps 0.1.0`.
- Future upgrades must select an explicit compatible pair from release notes or the `setup-calcit` validation matrix instead of resolving two independently released latest versions implicitly.
