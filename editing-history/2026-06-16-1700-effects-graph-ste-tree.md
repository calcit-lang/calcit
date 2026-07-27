# effects-graph STE Tree 格式改进

日期: 2026-06-16

## 问题

`cr analyze effects-graph` 默认 `sketch` 格式产出的是项目级鸟瞰图（调用树+聚合 effect/state），而非 RFC 预想的 per-function State/Transform/Effect 分解图。用户无法通过输出快速理解单个函数的功能。

## 改动

### 1. State 提取：从操作名改为目标名

`record_state_operator` 原先记录操作名（如 `swap!`），现改为提取目标 atom 名称（如 `*store`）。

- 新增 `extract_state_target(list, op_name)` 从 AST 列表中提取第一个参数
- `swap!/reset!/deref/set!` → 提取第一个参数名
- `defatom/atom` → 提取第一个参数名
- 修改 `walk_expr` → `inspect_call_head` 传参，传入完整 list

### 2. Effect 分类：添加 Respo 约定函数

`classify_by_name` 新增：
- `render-app!/mount-app!/rerender-app!/clear-cache!/realize-ssr!` → `render`
- `send-to-component!/dispatch!` → `lifecycle`
- `save-store!` → `storage`

`heuristic_effect_kinds` 优化：
- `unknown/effect!` → `effect`（更简洁）
- 函数名含 `load/init/setup` 时归类为 `io`

`set!` 加入 `is_state_operator` 和 early-return 列表（之前会被误分类为 effect）

### 3. 深度截断节点保留分析

原先 `max_depth` 到达时直接返回空壳节点（`empty_shell_node`）。
现改为：始终执行 `analyze_code` 提取 state/effects/transform，仅在 children 构建时跳过。
新增 `depth_exceeded` 字段标记截断节点。

### 4. STE Tree 格式（`--format tree`）

新增 `format_as_ste_tree` 函数，输出每个函数的 State/Transform/Effect 树形分解：

```
└── respo.main/main!  [program]
    ├── State
    │   ├── atom     *store (write)
    │   ├── return   unit
    │   └── watch    *store
    ├── Transform  (calls: 3)
    │   ├── → respo.app.core/render-app!
    │   └── → respo.main/save-store!
    └── Effects
        ├── console        println
        ├── render         render-app!
        └── storage        save-store!
```

- `[program]` = 含 effect 的函数
- `[transform]` = 纯变换函数
- `[depth limit ↑]` = 深度截断（有分析但无子节点）
- `(summary)` = 折叠节点的单行摘要（doc + effects）

`cr.rs` 中 `format=tree` 调度改为 `format_as_ste_tree`（原为 `format_for_llm` mermaid）。

### 5. 其他修复

- `birdview_state_labels` 匹配逻辑修复（kind 匹配而非 name 匹配）
- `effect_channel_name` 新增 `storage`/`lifecycle` 通道映射
- `ste_state_detail` 对 `return` 类型简化显示（仅类型，不显 "return" 前缀）
- `node_transform_summary` 修复 doc 与 summary 重复问题
- `empty_shell_node` 函数删除（不再使用）

## 涉及文件

- `src/effects_graph.rs` — 主要改动
- `src/bin/cr.rs` — tree 格式调度
- `src/cli_args.rs` — 无改动（格式字符串不变）

## 验证

- `cargo test effects_graph` — 5/5 通过
- `cargo clippy -- -D warnings` — 通过
- `yarn compile` — 通过
- `cr analyze effects-graph --format tree` on respo — 产出可用的 per-function STE 分解
