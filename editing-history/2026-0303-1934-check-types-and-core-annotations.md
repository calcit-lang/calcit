## 本次修改摘要

- 新增 `cr analyze check-types` 子命令，用于统计定义的类型标注覆盖度。
- 支持按命名空间过滤：`--ns`、`--ns-prefix`，以及是否包含依赖：`--deps`。
- 支持覆盖度分层与筛选：`none/partial/full`，并通过 `--only` 过滤输出。
- 优化输出格式：按定义类型（fn/macro/data）分块展示，参数与断言信息合并展示，`return-type` 标签统一为 `return`。
- 持续补充 `calcit.core` 中函数参数 `assert-type` 标注，`partial` 覆盖项降至 0（`--only partial` 无结果）。

## 验证

- 执行 `yarn check-all`：通过。
- 执行 `./target/debug/cr calcit/test.cirru analyze check-types --ns calcit.core --deps --only partial`：无 `partial` 定义。

## 经验记录

- 日常编辑可优先使用全局 `cr tree/edit`，减少 `cargo run` 反复编译开销。
- 若全局 `cr` 尚未包含新子命令（如 `analyze check-types`），统计/验证阶段需使用仓库内最新二进制（如 `./target/debug/cr`）。
