# 增加项目级严格迁移工作流

- 在现有 `calcit fix` 下增加 `--workflow strict`，组合 entry/type-slot、稳定安全改写、类型与 FFI review 边界和验证步骤，不新增顶层命令。
- `--apply` 强制绑定规划 revision 并复用 staged transaction；`--verify` 逐 entry 执行 keep-going 严格检查，并执行已声明的 verification profiles。
- manifest 使用稳定 token list 表示命令，Cirru EDN 为文档推荐格式；外部构建步骤保持显式空列表，不猜测包管理器。
- 增加 preview、apply、verify、参数冲突与 Agent structured-interface 回归测试。
