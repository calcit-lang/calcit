# 发布流程规范与加速

## 本次改动

- 更新 `Agents.md`：新增"发布流程规范"章节，记录版本升级、PR/tag 习惯、crates.io 加速措施
- 更新 `.github/workflows/publish.yaml`：加入两项加速
  1. `CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse`：加速 crates.io index 查找（分钟级 → 秒级）
  2. `cargo publish --no-verify`：跳过发布前重复的编译验证，因为本地和 PR CI 已跑过

## 关键规范

- 版本升级：`Cargo.toml` 和 `package.json` 两边同步改，再更新 `Cargo.lock`/`yarn.lock`
- Tag 格式：`0.12.27`（无 `v` 前缀），与 commit 一起打，message 注明版本号
- 分支开发，不自动合并；本地测试全过 → push PR → Actions 全过 → 手动合并
