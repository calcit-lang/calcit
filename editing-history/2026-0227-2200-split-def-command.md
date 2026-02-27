# 202602272200 — 新增 `edit split-def` 命令

## 改动概要

新增 `cr edit split-def <ns/def> -p <path> -n <new-name>` 命令，用于将某定义内指定路径的子表达式提取为同命名空间内的一个新定义，原位置替换为新定义的名字。

## 修改文件

- `src/cli_args.rs`：新增 `EditSplitDefCommand` struct（参数：`target`, `-p/--path`, `-n/--name`）；在 `EditSubcommand` 枚举中新增 `SplitDef(EditSplitDefCommand)` 变体。
- `src/bin/cli_handlers/edit.rs`：import 增加 `EditSplitDefCommand`；dispatch 增加 `EditSubcommand::SplitDef`；实现 `handle_split_def` 函数。
- `docs/CalcitAgent.md`：
  1. 修复 `tree unwrap` 示例（移除已废弃的 `-i 1` 参数，更新描述为 splice 所有子节点语义）。
  2. 在"结构化变更示例"中增加 `split-def` 的使用示例。
  3. 在"定义操作"列表中增加 `cr edit split-def` 条目。
  4. 新增"🔧 实战重构场景"章节，列举提取子表达式、rename、mv-def、mv/cp、unwrap/rewrite、批量批量重命名等常见重构操作的完整命令序列。

## 实现逻辑（`handle_split_def`）

1. 用 `navigate_to_path` 读取指定路径的子节点（`extracted`）。
2. 验证新名称在当前 ns 中不存在（不允许覆盖）。
3. 用 `apply_operation_at_path(..., "replace", Some(&leaf_new_name))` 将原定义的该路径替换为引用叶子节点。
4. 用 `CodeEntry::from_code(extracted)` 创建新定义，插入到同 ns 的 `defs` 中。
5. 保存 snapshot。

## 知识点

- `split-def` 只操作 AST，不会自动添加 import。如果新定义需要被其他 ns 引用，需手动 `cr edit add-import`。
- 路径索引规则与 `cr tree` 系列一致（逗号分隔，0-based）。
- 提取后如需给新定义包装成 `defn` 函数形式，用 `cr tree replace <ns/new-name> -p '' -e 'defn new-name (args...) ...'`。
