# 保留 fix staged validation 的 entry

## 问题

`calcit --entry browser calcit.cirru fix ...` 会先按 `browser` entry 规划改写，但 staged Snapshot 的验证子进程没有携带 entry，因而回落到 `default`。多 target 项目会用错误的 target、modules 或 type slots 验证合法改写。

## 修复

- staged validation 显式继承已加载 Snapshot 的 active entry。
- `--entry` 作为顶层参数放在临时 Snapshot 路径和 `fix` 子命令之前。
- 单元测试固定完整参数顺序；CLI 集成测试使用 Browser-only external-object method 复现多 target 场景。
- 文档明确 staged preprocess 与调用方使用同一个 entry 配置。
