# 2026-07-28 caps 模块状态检查与重置

## 修改概要

- 为 `caps` 增加 `status` 子命令，检查 `deps.cirru` 声明模块的版本、缺失状态和本地 Git 修改。
- 为 `caps` 增加 `reset` 子命令，重置已安装模块的 tracked 本地修改。
- 普通 `caps` 同步前增加本地模块状态提示。
- 补充依赖加载文档。

## 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`
