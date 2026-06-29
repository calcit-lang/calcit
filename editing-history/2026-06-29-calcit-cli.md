# calcit.cli / cr exec 完整实现

## 概要

实现 `cr exec` + `calcit.cli/*` 内建函数（共 77 个），让 Agent 用 Cirru heredoc 调用常用 `cr query` / `edit` / `tree` / `analyze` / `docs` 能力，绕开 Shell 转义。RFC：`RFCs/06-29-cr-exec-cli-builtins-rfc.md`。

## 架构

- **运行时**：`src/bin/injection/mod.rs` 注册 handler；**单一 options map 参数**（arity 1）
- **参数解析**：`calcit_cli_args.rs` — `resolve_cli_args`、类型 coercion、默认值填充、command echo
- **参数规格**：`calcit_cli_specs.rs` — 每个函数的 `CliArgSpec` 列表（语义化键名）
- **元数据**：`src/cirru/calcit-core.cirru` 的 `|calcit.cli` — schema 为 `(:args $ [] (:: :map :tag :dynamic))`
- **同步**：`python3 scripts/gen_calcit_cli_core.py`（从 mod.rs + calcit_cli_specs.rs 生成）
- **约束**：`calcit_cli*` 不得依赖 `cli_handlers`（保证 `cr-wasm` 可编译）

### 调用约定（2026-06-29 重构）

```cirru
calcit.cli/peek-def $ {} (:file-path |calcit.cirru) (:target |app.main/main!)
```

- 唯一参数为 options map；键为 tag（`:file-path`、`:target`、`:lines` 等）
- 省略的可选键由 Rust 侧默认值填充；非 quiet 模式下 stderr echo 展示完整调用（含默认值）
- 位置参数已移除（breaking change）

| 文件 | 职责 |
|------|------|
| `calcit_cli_args.rs` | map 解析、echo、docs_hint 生成 |
| `calcit_cli_specs.rs` | 77 个函数的参数 spec 常量 |
| `calcit_cli.rs` | 基础 query/write、snapshot 读写、`load_calcit_snapshot_with_deps` |
| `calcit_cli_tree.rs` | AST 树变换（delete/insert/wrap/cp/mv…） |
| `calcit_cli_extra.rs` | edit/config/cirru 工具、trigger-inc、semver bump |
| `calcit_cli_program.rs` | 加载 snapshot+modules+core 并 preprocess（analyze/validate） |
| `calcit_cli_analyze.rs` | call/effects/count + check-types/weak-types |
| `calcit_cli_query.rs` | show-pkg/ns、list-defs-by-tag、validate-file、search-*、list-host-procs |
| `calcit_cli_docs.rs` | docs-agents/read/sections |
| `src/type_coverage.rs` | check-types / weak-types 共享逻辑（`cr analyze` 与 calcit.cli 共用） |

## 分批功能（合并记录）

### 第一批：树/定义/import 写操作（17）

`tree-delete` `tree-insert` `tree-wrap` `tree-unwrap` `tree-raise` `tree-cp` `tree-mv` `rename-def` `rm-def` `mv-def` `split-def` `add-ns` `edit-doc` `edit-schema` `add-example` `rm-import` `show-error`

要点：`tree-wrap` 用 `self` leaf；`position` 同 CLI（`|before` `|after` 等）。

### 第二批：edit/config/cirru（20）

`rm-ns` `set-imports` `format-file` `show-doc` `show-ns-doc` `list-tags` `set-tags` `rm-example` `clear-examples` `set-examples` `tree-replace-leaf` `tree-swap-*` `tree-batch-delete` `tree-rewrite` `set-config` `add/rm-module` `cirru-parse/format` `read-text-file`

修复：`build_ns_code` 对齐 `(ns name (:require ...))` 嵌套结构。

### 第三批：analyze / inc / docs 搜索（9）

`trigger-inc` `cirru-parse-edn` `cirru-show-guide` `docs-search` `tree-replace-leaf-regex` `analyze-call-graph` `analyze-effects-graph` `analyze-count-calls`

### 第四批：P0–P3（15）

- **P0**：`show-pkg` `show-ns` `list-defs-by-tag` `validate-file` `bump-version` `edit-ns-doc`
- **P1**：`search-project` `search-def-regex` `search-expr` `list-host-procs`
- **P2**：`analyze-check-types` `analyze-weak-types`（依赖 `type_coverage` 提取）
- **P3**：`docs-agents` `docs-read` `docs-sections`

### Core 声明 + 预处理

- core 可选参数 schema 用 `(:rest :dynamic)`，勿写 `:rest $ :dynamic`（EDN 解析失败）
- 预处理：`is_registered_proc` 必须在 `has_def_code` 之前，避免 core 元数据 stub 被当作普通 def 编译（`preprocess/mod.rs`）

## 验证

```bash
python3 scripts/gen_calcit_cli_core.py
unset CARGO_TARGET_DIR && cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --bin cr
./target/debug/cr calcit/test.cirru exec << 'EOF'
calcit.cli/show-pkg $ {} (:file-path |calcit/test.cirru)
calcit.cli/validate-file $ {} (:file-path |calcit/test.cirru)
EOF
```
