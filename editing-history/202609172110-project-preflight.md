# 项目验证前置检查

- 在既有 `calcit analyze verify` 与 `calcit fix --workflow strict` 中加入共享的只读 preflight，没有增加顶层命令。
- Snapshot verification metadata 可声明 Node、Yarn、Rust、Caps 的 SemVer 要求，以及由调用方执行的 external gate 名称。
- 汇总 Snapshot entry/target/revision、Calcit、`deps.cirru`、`@calcit/procs`、`package.json`、安装结果和 Yarn lockfile 证据；版本矛盾以结构化诊断阻断验证。
- 外部 gate 只报告 `caller-required`，不执行 shell，不从项目文件猜测包管理器或构建命令。
- 更新验证 profile、严格迁移 workflow 与 Agent 文档，并覆盖显式 host mismatch 和 Yarn lockfile resolution 的 CLI 测试。
- 同步 core Snapshot 构建期结构，并把新增 binary 字段追加为带默认值的尾字段，保留旧 Snapshot 解码兼容性。
- review 后收紧显式声明边界：Caps 与其他 host 工具一样未声明不探测；重复规范化 tool key 直接拒绝；Node mismatch 测试使用隔离的确定性 fixture。
