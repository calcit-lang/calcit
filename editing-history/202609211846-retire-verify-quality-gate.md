# 声明式验证不再运行零债务质量预算

## 背景

`verify :quality` 在每个 entry 上重复运行无 baseline 的 `analyze quality`，按 Dynamic、nil 等统计数量决定成败。默认严格编译已经是类型正确性门槛；这一 profile check 既没有生产配置调用，也与 #1238 的收敛方向冲突。

## 调整

- `analyze verify` profile 当前只支持 `:strict`，继续覆盖多个 entry、preflight 与声明式外部门禁记录。
- 旧 `:quality` 配置在解析阶段明确提示改用 `:strict` 和行为测试；只有已有非零迁移 baseline 的项目才暂时显式使用 `analyze quality --baseline ...`。
- 测试覆盖配置迁移提示、strict 失败时的 `:stop`、跨 native/JS entry 的结构化输出；文档同步更新。这里改变的是 CLI 配置协议，因此测试保留在 Rust。

## 后续

core CI 的 quality baseline 与单独的显式质量分析仍待盘点迁移；本次不关闭 #1238。

## 验证

- `cargo test -q`、`cargo clippy -- -D warnings`、`cargo fmt --check` 通过。
- `yarn check-all` 通过，包含 304 个 Calcit definition 测试。
- `bash scripts/check-docs-md.sh` 通过，340 个文档代码片段均成功。
