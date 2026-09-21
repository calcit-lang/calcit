# 严格诊断不再隐含质量数量预算

## 背景

`--strict-types` 已不再是启用严格诊断的必要开关，但此前仍会隐式调用无 baseline 的 `analyze quality`，按 Dynamic、nil 和 `unsafe-coerce` 等命中数量决定是否失败。这与“类型正确性由编译器推断及 warning/error 判断”的方向冲突，也使 `--check-only --keep-going` 无法与显式严格模式组合。

## 调整

- 默认与显式严格模式共用预处理诊断；`--strict-types` 继续提供运行和代码生成前的入口预检查，并与 `--compat-types` 互斥。
- 移除严格检查、增量检查与 `check-public` 路径中的隐式零债务 quality gate。显式 `analyze quality` 和旧项目 baseline 暂时保留，待 #1238 的后续迁移单独清理。
- `--check-only --keep-going --strict-types` 现在输出同一份结构化诊断；已审阅的开放边界不会只因数量预算失败。
- 文档列出各 analyzer 的 owner 与后续收敛方向，避免把迁移报告解释成第二套类型系统。

## 验证

- `yarn check-all`、`yarn check-agent-interface`、`bash scripts/check-docs-md.sh` 通过。
- `cargo test -q --test strict_check_cli`、全量 `cargo test -q`、`cargo clippy -- -D warnings`、`cargo fmt --check` 通过；全量 Rust 测试运行时允许本地 HTTP fixture 绑定端口。
