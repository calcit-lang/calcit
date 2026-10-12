# cursor 导航 / 锚点 / 书签 / 剪贴板 / Paredit 合并入口（#1566 阶段 B）

## 实际阻塞与根因

`cursor` 组有 30 个叶子，其中 21 个只是同一动作族的不同拼写：七个移动叶子（parent、child、next、prev、forward、backward、back）、
锚点的 clear-anchor 与 region、书签的 mark / goto / rm-mark、剪贴板的 copy / cut / paste / clear-clipboard，
以及 slurp-next / slurp-prev / barf-last / barf-first / duplicate。`cursor apply` 已经用操作名表达树编辑，
Agent 却仍要记住 21 个叶子名，帮助文本也随之膨胀。

## 收敛方案

- 新增 `cursor move <direction>`，方向为 parent、child、next、prev、forward、backward、back，`--count` 与 child 的 `index` / `--last` 沿用旧叶子。
- `cursor anchor [set|clear|show]`、`cursor marks [list|set|go|drop] [name]`、`cursor clipboard [show|copy|cut|paste|clear] [--at]` 在原叶子上增加动作位置参数；
  不带动作时仍是原行为（set / list / show），所以这三个名字不弃用，只弃用被吸收的叶子。书签没有使用嵌套子命令：旧 `cursor mark <name>` 的名字参数会与子命令歧义，
  且 argh 要求只有最后一个位置参数可选，所以 `marks` 用 `Vec<String>` 位置参数加手写校验。
- `cursor apply` 增加 slurp-next、slurp-prev、barf-last、barf-first、duplicate 五个操作与 `--at`（仅 duplicate）。
- 所有新形式在 handle_cursor_command 入口被 normalize_cursor_subcommand 规范化为旧叶子结构，先于写锁与工具链守卫，因此未知方向 / 动作、
  缺失或多余的名字、不适用的 `--count` / `--at` / `--code` 在任何写入前报错；写入路径、暂存提交与 cursor 维护沿用旧实现。
- 旧叶子保留，帮助文本以 `[Deprecated]` 开头，stderr 输出一行迁移提示（在规范化之前计算，新入口不触发）。cursor 不在 `edit transaction` 操作集中，因此无事务提示。
- 未处理 push / pop（栈语义）与 set / show / clear；它们没有重复叶子，保持不变。未新增 `calcit fix` 规则：这些是命令行入口，没有可回写的 Snapshot 改写。

## 验证与边界

tests/deprecated_aliases_cli.rs 在两份独立 Snapshot 上分别重放新旧写法，逐步比较 Snapshot 文本与 `cursor show --format json`（去除临时目录路径）、旧写法恰好一行提示、新写法无提示；
另一个用例覆盖 17 种非法组合不写入也不移动 cursor。旧叶子在下一个非 patch 版本删除，迁移说明见 docs/run/upgrade.md。
