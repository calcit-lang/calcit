# 动态方法分析退役数量门槛

## 背景

`analyze dynamic-methods --max` 与 `analyze verify` 的 `:dynamic-methods` check 会按未静态分派的方法调用数量决定成败。默认严格预处理已对项目代码中的未证明分派报告类型错误，这两个门槛重复维护正确性语义，与 #1238 收敛 analyzer 的目标冲突。

## 调整

- `analyze dynamic-methods` 保留为可定位源码的只读报告，移除 `--max`、policy 结果和统计性失败诊断；JSON/EDN 上层 envelope 仍沿用已有输出协议。
- `verify` profile 不再接受 `:dynamic-methods`，旧配置会在解析阶段收到明确的迁移提示；`:strict` 与行为测试承担正确性验证。
- 升级指南与命令参考移除动态方法数量门槛示例，说明只读报告和严格检查的职责边界。
- Rust CLI 测试覆盖有实际动态方法 finding 时只读报告成功、旧 `--max` 拒绝，以及旧 profile check 的迁移错误。此处属于 CLI 协议和配置解析，不是新增语言语义，因此测试保留在 Rust。

## 后续

`analyze quality` 和 `verify :quality` 的存量质量预算，以及 core CI baseline，仍需在 #1238 后续阶段另行迁移；不能将本阶段视为 issue 已完成。

## 验证

- `cargo test -q`、`cargo clippy -- -D warnings`、`cargo fmt --check` 通过。
- `yarn check-all` 通过，其中包含 304 个 Calcit definition 测试与 Agent CLI 协议检查。
- `bash scripts/check-docs-md.sh` 通过，共 340 个文档代码片段。
