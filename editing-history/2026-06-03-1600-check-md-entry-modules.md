# 2026-06-03 check-md entry modules 与文档校验改进

- `cr docs check-md` 现在默认读取 `entry`（如 `demos/calcit.cirru`）里的 `configs.modules`，并与 CLI 的 `--dep` 合并去重，避免每次手动重复传依赖。
- `check-md` 的 `no-run` 模式在找不到默认入口定义时，增加了对 `app.*` 命名空间定义的回退编译检查，提升示例片段可检性。
- 更新 `check-md` 帮助文案，明确 `--dep` 是额外依赖，默认依赖来自 `entry configs.modules`。
- 新增单元测试覆盖 `entry modules + --dep` 合并去重逻辑，并通过 `docs check-md` 实测验证 `respo` 文档在不显式传 `--dep` 时可通过。
