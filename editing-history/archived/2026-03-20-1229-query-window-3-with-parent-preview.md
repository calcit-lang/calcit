# Query 搜索结果分层、降噪与路径/预览优化（合并记录）

## 变更概述

- 初始实现：为 `cr query` 搜索类命令引入结果分层展示（详情窗口 + 压缩输出）。
- 参数命名收敛：将 `--prefer` 直接改为 `--detail-offset`（不保留兼容别名）。
- 输出降噪：删除冗长提示，保留定位核心信息（定义、路径、命中片段）。
- 路径统一：搜索输出与 `--start-path` 示例统一为点号分隔；解析层仅接受点号输入。
- 详情窗口进一步从 5 缩小为 3，并在窗口内增加“所在表达式 + 父表达式”预览（可省略复杂父表达式）。

## 覆盖命令

- `cr query find`
- `cr query usages`
- `cr query search`
- `cr query search-expr`

## 关键实现

- `src/cli_args.rs`
  - 参数字段统一为 `detail_offset`。
  - 参数统一为 `#[argh(option, long = "detail-offset", default = "0")]`。
  - 参数说明更新为“3 detailed items”。
  - `query search --start-path` 示例更新为点号格式 `2.1.0`。

- `src/bin/cli_handlers/query.rs`
  - 结果窗口逻辑：
    - `DETAILED_RESULTS_WINDOW = 3`
    - `detailed_window(detail_offset, total)`
    - `in_detail_window(index, total, detail_offset)`
    - `print_detail_window_hint(...)`
  - 搜索结果降噪：
    - `search/search-expr` 按命中数排序定义（高命中优先）。
    - 详情行输出为 `[path] <preview>`；窗口外仅输出压缩条数（`N matches compressed outside window`）。
    - 移除 `Next steps`/批量替换等长提示块。
  - 预览增强：
    - 新增 `preview_node_oneline(...)` 并修复叶子节点空预览问题。
    - 新增 `path_parent(...)`、`get_node_at_path(...)`、`count_nodes_limited(...)`、`can_show_parent_preview(...)`、`expression_and_parent_preview(...)`。
    - 最终形态改为“单行预览”：优先显示父节点预览；无父节点时回退到当前表达式（避免重复展示两遍）。

  - Tips 与 exact 语义修正：
    - `--exact` 高优先级提示文案统一为英文：`Many matches (N); add --exact to show exact matches only`。
    - 仅在 contains 模式且命中数 `> 10` 时展示该提示；`--exact` 模式不再显示冗余提示。
    - 修复 exact 模式下的误导性高亮：只在 token 边界高亮，避免将 `states` 中的 `state` 误标为命中。

  - 命名收敛：将旧命名 `prefer` 在函数参数/调用链/提示文案中统一改为 `detail_offset` / `detail-offset`。

- `src/bin/cli_handlers/common.rs`
  - `parse_path(...)` 改为仅接受点号分隔；逗号输入报错并提示改用点号。

## 降噪策略

- 深度嵌套（路径过深）或分支复杂（节点过大）时，父表达式预览自动省略。
- 详细窗口外仍然输出压缩占位，不展开上下文。

## 验证

- `cargo check --bin cr` 通过。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query find --help`
  - 可见 `--detail-offset` 参数。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query search --help`
  - `--start-path` 示例为点号格式。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query search state --filter respo.app.comp.todolist/comp-todolist --detail-offset 5`
  - 输出降噪，路径为点号，窗口外压缩。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query search state --filter respo.app.comp.todolist/comp-todolist --detail-offset 0`
  - 详情窗口为 `[0, 3)`，每条命中仅展示一行（优先父节点预览）。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query search state --filter respo.app.comp.todolist/comp-todolist --detail-offset 0 --exact`
  - 精确命中不再把 `states` 视觉误判为 `state`；且不显示“add --exact”提示。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query search-expr state --filter respo.app.comp.todolist/comp-todolist --detail-offset 0`
  - 输出样式一致（单行预览）。
- `cargo run --bin cr -- /Users/jon.chen/repo/respo/respo/calcit.cirru query search-expr state --filter respo.app.comp.todolist/comp-todolist --detail-offset 0 --exact`
  - 结果为 `No matches found`，符合结构精确匹配预期。
