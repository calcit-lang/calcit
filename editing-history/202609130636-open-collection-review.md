# 补充开放集合泛型检查顺序说明

时间：2026-09-13 06:36 +0800

## 调整

- 根据 #1046 review，在 generic argument 排序处补充英文源码注释。
- 明确先检查 concrete type evidence 是为了先建立泛型绑定，再让开放 `Dynamic` 参数复用该证明。
- 不改变已有实现与类型语义。

## 验证

- `cargo fmt --all -- --check`
- `cargo test find_unproven_generic_argument`
