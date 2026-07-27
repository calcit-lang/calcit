# calcit.cli 与 CLI handlers 重复代码分析与重构方案

时间: 2026-06-30
作者: AI Agent

## 现状

### 两套独立实现

| 方面 | `injection/calcit_cli*.rs` | `cli_handlers/*.rs` |
|------|---------------------------|---------------------|
| 代码量 | ~2714 行 | ~5967 行 |
| 入口 | `fn(xs: Vec<Calcit>, _: &CallStackList) -> Result<Calcit, CalcitErr>` | `fn(opts: &SpecificCommandStruct, snapshot_file: &str) -> Result<(), String>` |
| 参数解析 | `resolve_cli_args()` → `ResolvedCliArgs` (从 Calcit Map 中提取) | argh 自动解析 CLI 参数到结构体 |
| 输出 | `Calcit::Str(...)` 返回给 Calcit 运行时 | `println!()` 直接输出 |
| 快照加载 | `load_calcit_snapshot()` | `load_snapshot()` |
| 快照保存 | `save_calcit_snapshot()` | `save_snapshot()` |

### 完全重复的辅助函数

| 函数 | injection/ 位置 | cli_handlers/ 位置 |
|------|----------------|-------------------|
| load_snapshot | calcit_cli.rs:1155 | edit.rs:101 |
| save_snapshot | calcit_cli.rs:1217 | edit.rs:109 |
| parse_path | calcit_cli.rs:933 | common.rs:182 |
| navigate_to_node | calcit_cli.rs:905 | edit.rs:1267 |
| parse_target | calcit_cli_args.rs (复用 cli_options) | edit.rs:22 |
| check_ns_editable | calcit_cli.rs:1033 | edit.rs:114 |
| apply_operation_at_path | calcit_cli_tree.rs | edit.rs:1152 |
| process_node_with_references | calcit_cli_tree.rs | edit.rs:1290 |

## 推荐的桥接方案

### 方案: 提取共享核心 + 让 calcit.cli 代理到 cli_handlers

不直接让 `calcit.cli` procs 调用 `cli_handlers`（因为接口类型不同），而是:

1. **将核心业务逻辑（load → modify → save）提取到共享函数**，放在 `snapshot.rs` 或新建 `src/snapshot_ops.rs`
2. **`cli_handlers` 调用共享函数**
3. **`calcit.cli` 内部转发到共享函数**，而不是自己再实现一遍

### 关键接口对齐

calcit.cli proc 的处理流程:
```
Vec<Calcit> → resolve_cli_args() → ResolvedCliArgs → 提取参数 → 业务逻辑 → Calcit
```

CLI handler 的处理流程:
```
CLI struct → 提取参数 → 业务逻辑 → println!
```

共同点: 都需要 `(file_path, target, path, code)` 等参数, 都调用 `load_snapshot` / `save_snapshot` / `navigate_to_path` / `apply_operation_at_path`

### 具体步骤

1. 把 `load_snapshot`, `save_snapshot`, `navigate_to_path`, `parse_path`, `check_ns_editable`, `apply_operation_at_path`, `process_node_with_references` 等辅助函数统一放到 `src/snapshot_ops.rs` (或 `src/bin/cli_handlers/ops.rs`)
2. 把每个业务操作（如 tree-replace、show-def、list-defs）提取为纯函数:
   ```rust
   pub fn op_tree_replace(snapshot: &mut Snapshot, ns: &str, def: &str, path: &[usize], new_node: &Cirru) -> Result<(), String>
   pub fn op_show_def(snapshot: &Snapshot, ns: &str, def: &str) -> Result<String, String>
   ```
3. `calcit.cli` 的每个 proc handler 变成薄薄的代理层:
   ```rust
   pub fn tree_replace(xs: Vec<Calcit>, _cs: &CallStackList) -> Result<Calcit, CalcitErr> {
     let args = resolve_cli_args("calcit.cli/tree-replace", &xs, TREE_REPLACE)?;
     let mut snapshot = load_snapshot(&args.file_path()?)?;
     let (ns, def) = args.target("target")?;
     let path = parse_path(&args.string("path")?, false)?;
     let code = args.cirru_quote("code")?;
     ops::op_tree_replace(&mut snapshot, &ns, &def, &path, &code)?;
     save_snapshot(&snapshot, &args.file_path()?)?;
     Ok(Calcit::Str(Arc::from("replaced")))
   }
   ```
4. CLI handlers 也变成薄代理:
   ```rust
   fn handle_replace(opts: &TreeReplaceCommand, snapshot_file: &str) -> Result<(), String> {
     let (ns, def) = parse_target(&opts.target)?;
     let path = parse_path(&opts.path)?;
     let code = parse_input_to_cirru(...)?;
     let mut snapshot = load_snapshot(snapshot_file)?;
     ops::op_tree_replace(&mut snapshot, ns, def, &path, &code)?;
     save_snapshot(&snapshot, snapshot_file)?;
     // print preview...
   }
   ```

### 收益
- 消除 ~2000 行重复代码
- 修复 bug 只需改一处
- 新功能只需写一次共享逻辑

### 风险
- 大规模重构需要充分测试
- 需要迁移 `calcit_cli_tree.rs` 中的复杂逻辑（rewrite with references）
- `calcit.cli` 的 `CalcitErr` 错误类型和 `cli_handlers` 的 `String` 错误类型需要对齐

## 代码规模统计

```
src/bin/injection/
  calcit_cli.rs         1297 行  ← 需要拆解: 辅助函数 + proc handlers
  calcit_cli_extra.rs    846 行  ← 同上
  calcit_cli_tree.rs     232 行  ← 纯辅助函数, 可整体移出
  calcit_cli_args.rs     147 行  ← 桥接层, 保留
  calcit_cli_specs.rs    192 行  ← 参数规格定义, 保留
  mod.rs                 ~60 行  ← 注册代码, 保留

src/bin/cli_handlers/
  edit.rs              2026 行  ← 辅助函数 + handler 逻辑
  tree.rs              1433 行  ← handler 逻辑
  query.rs             2135 行  ← handler 逻辑
  common.rs             373 行  ← 工具函数
```
