# 202605210055 — tree show 可选路径 + cr config 顶级命令 + cr edit 清理

## 修改概要

### 1. `cr tree show` — `-p` 参数改为可选，缺失时警告而非报错

**动机**: Agent 调用 `cr tree show ns/def` 时若忘记 `-p`，原来直接报错导致 token 浪费。

**变更文件**:
- `src/cli_args.rs`: `TreeShowCommand.path` 类型从 `String` → `Option<String>`，doc 注释更新
- `src/bin/cli_handlers/tree.rs`: `handle_show` 中对 `None` 输出黄色 `[Warn]`，默认使用空路径（root）
- `src/bin/cli_handlers/command_echo.rs`: `push_tree` Show 分支改用 `opt "path"` 宏处理 Option 类型

**效果**:
```
$ cr calcit.cirru tree show "ns/def"
[Warn] No path (-p) specified; showing from root. Use -p '0' to start from a child node.
Type: list (4 items)
...
```

---

### 2. `cr config` — 顶级配置管理命令

**动机**: `cr query config`、`cr query modules`、`cr edit config`、`cr edit add-module`、`cr edit rm-module` 分散在不同子命令下，操作配置需要记忆多个入口。新增统一入口 `cr config`。

**新命令**:
| 命令 | 等价旧命令 |
|---|---|
| `cr config show` | `cr query config` |
| `cr config modules` | `cr query modules` |
| `cr config version [value]` | `cr edit config version ...` |
| `cr config set <key> <value>` | `cr edit config <key> <value>` |
| `cr config add-module <path>` | `cr edit add-module <path>` |
| `cr config rm-module <path>` | `cr edit rm-module <path>` |

`cr config version` 可独立使用（无参数显示当前版本，传 `patch/minor/major` 递增，传 semver 直接设置）。

**变更文件**:
- `src/cli_args.rs`: 添加 `CalcitCommand::Config`、`ConfigCommand`、`ConfigSubcommand`（含 `ConfigVersionCommand`）及各子命令 struct
- `src/bin/cli_handlers/config.rs`: 新建，实现 `handle_config_command` 及各子命令处理函数（含 `handle_version`）
- `src/bin/cli_handlers/edit.rs`: `parse_semver_value`、`bump_semver_value` 改为 `pub(crate)`
- `src/bin/cli_handlers/mod.rs`: 注册 `mod config` 并 re-export `handle_config_command`
- `src/bin/cli_handlers/command_echo.rs`: 添加 `config_name`、`push_config`，并在 `should_echo_command` / `render_command_echo` 中处理 `Config` 变体
- `src/bin/cr.rs`: 添加 `CalcitCommand::Config` 分支

---

### 3. `cr edit` 清理 — 移除已迁移至 `cr config` 的子命令

**动机**: `cr edit add-module`、`cr edit rm-module`、`cr edit config` 与新增的 `cr config` 重复，保持单一职责。

**删除内容**:
- `EditSubcommand::AddModule / RmModule / Config` 及对应 struct（`cli_args.rs`）
- `handle_add_module / handle_rm_module / handle_config` 函数体（`edit.rs`）
- `command_echo.rs` 中 `edit_name` 和 `push_edit` 两处 match 的三个变体

**保留**（被 `config.rs` 和单元测试引用）:
- `pub(crate) fn parse_semver_value` — semver 字符串解析
- `pub(crate) fn bump_semver_value` — patch/minor/major 递增

**文档更新**:
- `docs/run/agent-advanced.md` "模块和配置" 章节、快速参考示例 → 全部改为 `cr config` 命令
- `docs/CalcitAgent.md` 第 9 节"cr 能力地图" → 新增"配置管理"条目

---

## 知识点

- `argh` 中将 `#[argh(option)]` 字段类型改为 `Option<T>` 可使该选项变为可选
- `pub(crate)` 可在同一 crate 内跨模块共享；被其他模块引用时，不能因删除调用方而一并删除
- 新增顶级命令时需同步更新 `command_echo.rs` 的三处：`should_echo_command`、`render_command_echo`（名称映射和 token push）
- 移除 enum 变体时，需同时检查 `command_echo.rs` 的两处 match（`edit_name` 和 `push_edit`）
- 单元测试也会引用函数，删除前要 grep `tests` 模块确认
