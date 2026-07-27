## CLI 改进: target-replace → search-replace, 新增 batch-delete, --parent-path

### 改动机
根据 pudica-schedule-viewer 升级过程中暴露的多次 cr 命令使用痛点:

1. `target-replace` 命名不直观（"目标替换"不如"搜索替换"清晰）
2. 连续删除多个属性时需要手动从高到低删除，容易出错
3. 搜索结果的路径是叶子路径，用户需要手动去掉末尾索引得到可编辑父路径
4. map 属性中用 `$` 编辑的规则未在文档中明确

### 改动清单

**src/cli_args.rs:**
- `TargetReplace(TreeTargetReplaceCommand)` → `SearchReplace(TreeSearchReplaceCommand)`
- `#[argh(subcommand, name = "target-replace")]` → `#[argh(subcommand, name = "search-replace")]`
- 新增 `BatchDelete(TreeBatchDeleteCommand)` 枚举变体
- 新增 `TreeBatchDeleteCommand` 结构体（`-p` 接受多个路径）
- `QuerySearchCommand` 新增 `--parent-path` 开关

**src/bin/cli_handlers/tree.rs:**
- `handle_target_replace` → `handle_search_replace`
- 新增 `handle_batch_delete`: 按路径降序依次调用 `handle_delete`

**src/bin/cli_handlers/query.rs:**
- `SearchCommonOpts` 新增 `parent_path: bool` 字段
- 搜索结果中 `--parent-path` 开启时额外打印父路径

**src/bin/cli_handlers/command_echo.rs:**
- 更新所有 `TargetReplace` → `SearchReplace` 分发
- 新增 `BatchDelete` 的三个 match arm

**src/bin/cli_handlers/edit.rs:**
- 错误提示 `target-replace` → `search-replace`

**src/bin/cli_handlers/docs_tests.rs:**
- 测试数据中 `target-replace` → `search-replace`

**docs/CalcitAgent.md + 其他文档:**
- 所有 `target-replace` 替换为 `search-replace`
- 新增 `batch-delete` 和 `--parent-path` 用法说明
- 新增 `$` 在属性 map 中的编辑指南
