# 语义化树形导航实现 (2026-07-07)

## 变更概要

实现 RFC `07-06-semantic-tree-navigation-rfc.md` 全部四个 Phase。

## 实现功能

### L1: `--path-annotations`
- `cr tree show` 新增 `--path-annotations` flag
- 开启后递归为所有嵌套 `Cirru::List` 末尾追加 `; "previous node path: X.Y.Z"` 注释节点
- AST 层面操作：`children.push(Cirru::List([Cirru::Leaf(";"), Cirru::Leaf(path_string)]))`
- 子节点 > 8 时底部 tip 提示可开启

### L2: `--pick` 多候选选择
- `search-replace` 多匹配时展示带路径、上下文的候选列表
- `--pick <N>` 直接选择第 N 个候选执行替换

### L4: 语义路径表达式
- `cr query path <ns> --selector 'path heading ... nth ...'`
- 三个选择器：裸叶子（精确匹配）、`heading`（前缀匹配）、`nth`（位置导航）
- `resolve_path_expression` 函数解析并返回数字路径

### L4: `--selector` 限定搜索范围
- `search-replace --selector 'path ...'` 在语义路径定位的子树内搜索替换
- 与 `cr query path` 共用同一套选择器语法

### Phase 4: `cr query anchors`
- 遍历 AST 中 `noted @anchor:<name>` 调用，输出锚点名称和路径

## 修改文件

| 文件 | 变更 |
|------|------|
| `src/cli_args.rs` | 新增 `path_annotations`, `pick`, `selector` 字段；新增 `QueryPathCommand`, `QueryAnchorsCommand` |
| `src/bin/cli_handlers/tree.rs` | `annotate_paths` 函数；`handle_show` 路径标注 + tip；`handle_search_replace` 多候选 + selector |
| `src/bin/cli_handlers/query.rs` | `resolve_path_expression`(pub), `starts_with_pattern`, `handle_query_path`, `handle_query_anchors`, `find_anchors` |
| `src/bin/cli_handlers/command_echo.rs` | Path/Anchors 子命令 echo 支持 |
| `docs/CalcitAgent.md` | 精简引用新功能 |
| `docs/run/agent-advanced.md` | 详细示例文档 |
| `RFCs/07-06-semantic-tree-navigation-rfc.md` | 新增 RFC |

## 设计决策

- `--at`/`--child` 链式语法被 `--selector` 替代，代码中已移除
- `exact` 选择器不纳入——链式导航模型不需要独立精确匹配，`heading` 无多余子节点时等同
- `sub-path` 选择器不纳入——L1 路径标注让 `nth` 足够
- 路径注释放在 list 末尾（不破坏已有子节点索引）
- `Cirru::Leaf` 存原始内容，格式化时自动加引号
