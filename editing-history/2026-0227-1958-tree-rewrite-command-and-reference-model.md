# 2026-02-27 19:58

## 知识点

- `tree structural` 改名为动词化的 `tree rewrite`，更贴近“结构重写”语义，也更容易让 AI 模型理解用途。
- `rewrite` 明确为**引用驱动**命令：必须提供至少一个 `--with name=path`，否则应提示改用 `tree replace`。
- 结构引用从旧的 `--refer-*` 模型统一为 `--with` 映射模型（如 `--with self=.`, `--with rhs=2`），减少参数心智负担。

## 改动概要

- CLI 路由与文案：`TreeSubcommand::Structural` 改为 `TreeSubcommand::Rewrite`，子命令名改为 `rewrite`。
- `tree` handler：将 `handle_structural` 更名为 `handle_rewrite`，并更新输出提示与错误信息。
- 结构引用处理：在 `tree` 中统一使用 `parse_with_references` + `process_node_with_references`。
- 文档更新：`docs/CalcitAgent.md` 与 `Agents.md` 全部改用 `tree rewrite` 与 `--with` 示例。
- 真实命令验证（基于 `demos/calcit.cirru`）：
  - `tree replace` 可执行普通替换；
  - `tree rewrite` 无 `--with` 会按预期报错；
  - `tree rewrite` 携带 `--with` 可按预期执行。
