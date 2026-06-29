# `cr exec` + `calcit.cli/*` 内建函数方案

> 状态: Active · 创建: 2026-06-29

## 摘要

本方案新增 `cr exec` 子命令，从 stdin 读取 Cirru snippet 并按 `cr eval` 的同一条预处理/执行链路运行。同时注册一组 `calcit.cli/*` 原生内建函数，把常用 `cr query`、`cr tree`、`cr edit` 能力暴露为 Cirru 函数调用。

核心目标是让 Agent 在需要批量查询或局部编辑 `.cirru` 文件时，用 heredoc/pipe 传入纯 Cirru 代码，绕开 Shell 对 `$`、`` ` ``、`|`、`>`、`<`、`&`、`;`、`(`、`)`、`!`、`?`、`*`、`[`、`]`、`{`、`}` 等字符的转义负担。

## 决策

- 新增 `cr <file> exec`，固定从 stdin 读取 snippet；短表达式仍使用 `cr <file> eval '<code>'`。
- `cr exec` 默认与 `cr eval` 一样走 check-only 预验证，再执行 `app.main/main!`。
- `--dep` 支持与 `eval` 对齐，用于显式加载外部模块。
- `calcit.cli/*` 以 registered proc 方式注册，命名采用全名如 `calcit.cli/list-ns`，不依赖用户在 snippet 中声明 `ns :require`。
- 文档中的 ` ```cirru.cli ` 代码块只做解析 + 预处理校验，不执行真实文件读写。

## 非目标

- 不恢复 `cr edit` / `cr tree` 的 stdin 输入；写入复杂代码仍优先用文件参数或结构化 API。
- 不把所有 CLI 子命令一次性映射为内建函数；MVP 只覆盖 Agent 高频查询和少量明确边界的写操作。
- 不承诺 `calcit.cli/*` 返回值格式完全等同传统 CLI 的人类可读输出；内建函数应优先返回可组合的 Calcit 值或稳定字符串。
- 不在 Calcit 语言层面引入新的 import 语法；`calcit.cli/*` 是 host 注册能力。

## 架构

```text
用户输入 (heredoc/pipe)
       |
       v
     stdin
       |
       v
    cr exec
       |
       v
snapshot::create_file_from_snippet()
       |
       v
预处理 + check-only 预验证 + eval
       |
       v
calcit.cli/* registered proc
       |
       v
读取/修改 .cirru snapshot 文件
```

关键点：

- `cr exec` 与 `cr eval` 共用 snippet 装载逻辑。若 snippet 首表达式是 `ns`，其 import 规则会合并进运行用的 `app.main`。
- `calcit.cli/*` 函数通过 `register_import_proc_with_descriptor` 注册；调用形式为 `calcit.cli/f $ {} (:key value) ...`，选项键在 `calcit_cli_specs.rs` 中声明。
- 预处理阶段需要在 `ns/def` 拆分失败路径之外识别 registered proc 全名，避免 `calcit.cli/list-ns` 被误判为普通 `ns/def` 引用。
- 写函数直接修改目标 snapshot 文件，属于有副作用的 host capability，必须保持 Native-only 和 Experimental 标记。

## MVP 能力

### 调用约定

所有 `calcit.cli/*` 函数统一采用 **map 选项** 调用，不再接受旧版 positional 字符串参数：

```cirru
calcit.cli/list-ns $ {} (:file-path |calcit.cirru)
calcit.cli/peek-def $ {} (:file-path |calcit.cirru) (:target |app.main/main!) (:lines 5)
calcit.cli/list-defs $ {} (:file-path |calcit.cirru) (:namespace |app.main)
calcit.cli/search-def $ {} (:file-path |calcit.cirru) (:target |app.main/main!) (:keyword |keyword)
calcit.cli/tree-show $ {} (:file-path |calcit.cirru) (:target |app.main/main!) (:path |3.1)
calcit.cli/show-error $ {}
calcit.cli/cirru-show-guide $ {}
```

常用选项键：

| 键 | 含义 |
| --- | --- |
| `file-path` | snapshot 文件路径（旧版第 1 个字符串参数） |
| `target` | `ns/def` |
| `namespace` | 仅命名空间 |
| `keyword` / `pattern` / `symbol` / `path` / `code` | 各函数语义参数 |
| `lines` / `max-lines` / `overwrite` 等 | 可选修饰（见各函数 spec） |

### 查询函数

| 函数                       | map 选项键                              | 等价 CLI            | 返回形态                         |
| -------------------------- | --------------------------------------- | ------------------- | -------------------------------- |
| `calcit.cli/list-ns`       | `file-path`                             | `cr query ns`       | namespace 字符串列表             |
| `calcit.cli/list-defs`     | `file-path` `namespace`                 | `cr query defs`     | definition 字符串列表            |
| `calcit.cli/show-def`      | `file-path` `target`                    | `cr query def`      | Cirru 代码字符串                  |
| `calcit.cli/peek-def`      | `file-path` `target` `[lines]`          | `cr query peek`     | Cirru 代码字符串                  |
| `calcit.cli/search-def`    | `file-path` `target` `keyword`          | `cr query search`   | `path leaf-preview` 字符串列表，如 `3.1.0 \|foo` |
| `calcit.cli/find-symbol`   | `file-path` `symbol`                    | `cr query find`     | `ns/def` 字符串列表              |
| `calcit.cli/show-schema`   | `file-path` `target`                    | `cr query schema`   | schema 字符串                    |
| `calcit.cli/list-examples` | `file-path` `target`                    | `cr query examples` | example 字符串列表               |
| `calcit.cli/list-usages`   | `file-path` `target`                    | `cr query usages`   | usage 字符串列表                 |
| `calcit.cli/list-config`   | `file-path`                             | `cr query config`   | config 字符串                    |
| `calcit.cli/list-modules`  | `file-path`                             | `cr config modules` | module 字符串列表                |
| `calcit.cli/tree-show`     | `file-path` `target` `[path]` `[max-lines]` | `cr tree show`   | Cirru 子树字符串；`max-lines` 控制输出行数 |

### 写函数

| 函数                        | map 选项键                                   | 等价 CLI                 | 约束                               |
| --------------------------- | -------------------------------------------- | ------------------------ | ---------------------------------- |
| `calcit.cli/edit-def`       | `file-path` `target` `code` `[overwrite]`    | `cr edit def`            | 只创建/覆盖单个定义                |
| `calcit.cli/tree-replace`   | `file-path` `target` `path` `code`           | `cr tree replace`        | path 必须完整、合法且不能静默降级  |
| `calcit.cli/search-replace` | `file-path` `target` `pattern` `replacement` | `cr tree search-replace` | 只替换完整 leaf，不做子串替换      |
| `calcit.cli/add-import`     | `file-path` `namespace` `source-ns` `refer-sym` | `cr edit add-import`  | MVP 只支持 `:refer` 单符号导入     |

### 扩展写函数（2026-06-29 增补）

| 函数                        | map 选项键                                                   | 等价 CLI                 |
| --------------------------- | ------------------------------------------------------------ | ------------------------ |
| `calcit.cli/tree-delete`    | `file-path` `target` `path`                                  | `cr tree delete`         |
| `calcit.cli/tree-insert`    | `file-path` `target` `path` `code` `position`                | `cr tree insert-*`       |
| `calcit.cli/tree-wrap`      | `file-path` `target` `path` `wrapper-code`                   | `cr tree wrap`           |
| `calcit.cli/tree-unwrap`    | `file-path` `target` `path`                                  | `cr tree unwrap`         |
| `calcit.cli/tree-raise`     | `file-path` `target` `path`                                  | `cr tree raise`          |
| `calcit.cli/tree-cp`        | `file-path` `target` `from-path` `to-path` `position`        | `cr edit cp`             |
| `calcit.cli/tree-mv`        | `file-path` `target` `from-path` `to-path` `position`        | `cr edit mv`             |
| `calcit.cli/rename-def`     | `file-path` `target` `new-name`                              | `cr edit rename`         |
| `calcit.cli/rm-def`         | `file-path` `target`                                         | `cr edit rm-def`         |
| `calcit.cli/mv-def`         | `file-path` `source` `target`                                | `cr edit mv-def`         |
| `calcit.cli/split-def`      | `file-path` `target` `path` `new-name`                       | `cr edit split-def`      |
| `calcit.cli/add-ns`         | `file-path` `namespace` `[code]`                           | `cr edit add-ns`         |
| `calcit.cli/rm-import`      | `file-path` `namespace` `source-ns`                          | `cr edit rm-import`      |
| `calcit.cli/edit-doc`       | `file-path` `target` `doc`                                   | `cr edit doc`            |
| `calcit.cli/edit-schema`    | `file-path` `target` `schema-code`                           | `cr edit schema`         |
| `calcit.cli/add-example`    | `file-path` `target` `code` `[index]`                        | `cr edit add-example`    |
| `calcit.cli/show-error`     | `[error-file=.calcit-error.cirru]`                           | `cr query error`         |

### 扩展写/工具函数（第二批）

| 函数                        | map 选项键                                                   | 等价 CLI                 |
| --------------------------- | ------------------------------------------------------------ | ------------------------ |
| `calcit.cli/rm-ns`          | `file-path` `namespace`                                      | `cr edit rm-ns`          |
| `calcit.cli/set-imports`    | `file-path` `namespace` `rules-code`                         | `cr edit imports`      |
| `calcit.cli/format-file`    | `file-path`                                                  | `cr edit format`         |
| `calcit.cli/show-doc`       | `file-path` `target`                                         | query doc 字段           |
| `calcit.cli/show-ns-doc`    | `file-path` `namespace`                                      | query ns doc             |
| `calcit.cli/list-tags`      | `file-path` `target`                                         | `cr edit tags`（读）     |
| `calcit.cli/set-tags`       | `file-path` `target` `tags`                                  | `cr edit tags`（写）     |
| `calcit.cli/rm-example`     | `file-path` `target` `index`                                 | `cr edit rm-example`     |
| `calcit.cli/clear-examples` | `file-path` `target`                                         | `cr edit examples --clear` |
| `calcit.cli/set-examples`   | `file-path` `target` `examples-code`                         | `cr edit examples`       |
| `calcit.cli/tree-replace-leaf` | `file-path` `target` `pattern` `replacement-code`         | `cr tree replace-leaf`   |
| `calcit.cli/tree-swap-next` | `file-path` `target` `path`                                  | `cr tree swap-next`      |
| `calcit.cli/tree-swap-prev` | `file-path` `target` `path`                                  | `cr tree swap-prev`      |
| `calcit.cli/tree-batch-delete` | `file-path` `target` `paths`                              | `cr tree batch-delete`   |
| `calcit.cli/tree-rewrite`   | `file-path` `target` `path` `template-code` `refs`           | `cr tree rewrite`        |
| `calcit.cli/set-config`     | `file-path` `key` `value` `[entry]`                          | `cr config set`          |
| `calcit.cli/add-module`     | `file-path` `module-path` `[entry]`                          | `cr config add-module`   |
| `calcit.cli/rm-module`      | `file-path` `module-path` `[entry]`                          | `cr config rm-module`    |
| `calcit.cli/cirru-parse`    | `code` `[one-liner]`                                         | `cr cirru parse`         |
| `calcit.cli/cirru-format`   | `json`                                                       | `cr cirru format`        |
| `calcit.cli/read-text-file` | `path`                                                       | 读任意文本（含 docs）    |

### 扩展分析/工具函数（第三批）

| 函数                              | map 选项键                                                       | 等价 CLI                      |
| --------------------------------- | ---------------------------------------------------------------- | ----------------------------- |
| `calcit.cli/trigger-inc`          | `file-path` `[changed]` `[added]` `[removed]` `[added-ns]` …     | `cr edit inc`                 |
| `calcit.cli/cirru-parse-edn`      | `edn`                                                            | `cr cirru parse-edn`          |
| `calcit.cli/cirru-show-guide`     | —                                                                | `cr cirru show-guide`         |
| `calcit.cli/docs-search`          | `keyword` `[docs-dir]`                                           | `cr docs search`（简化 grep） |
| `calcit.cli/tree-replace-leaf-regex` | `file-path` `target` `regex` `replacement-code`               | `cr tree replace-leaf --regex` |
| `calcit.cli/analyze-call-graph`   | `file-path` `[root]` `[format]` `[max-depth]` …                  | `cr analyze call-graph`       |
| `calcit.cli/analyze-effects-graph`| `file-path` `[root]` `[format]` `[max-depth]` …                  | `cr analyze effects-graph`    |
| `calcit.cli/analyze-count-calls`  | `file-path` `[root]` `[format]` …                                | `cr analyze count-calls`      |

`trigger-inc` 各 CSV 选项（如 `changed`）为逗号分隔的 `ns/def` 或 `ns`；写入当前目录 `.compact-inc.cirru`（与 `cr edit inc` 相同）。

`analyze-*` 会先加载 snapshot、模块与 core 并预处理，返回文本或 JSON 字符串（不打印到 stdout）。

`tree-insert` / `tree-cp` / `tree-mv` 的 `position` 取值：`before`、`after`、`prepend-child`、`append-child`、`replace`（字符串，如 `|after`）。

`tree-wrap` 的 wrapper 模板中用 `self` leaf 引用被包裹的原始节点。

### 扩展查询/写/分析/文档（第四批 2026-06-29）

| 函数 | map 选项键 | 等价 CLI |
| ---- | ---- | -------- |
| `calcit.cli/show-pkg` | `file-path` | `cr query pkg` |
| `calcit.cli/show-ns` | `file-path` `namespace` | `cr query ns <ns>` |
| `calcit.cli/list-defs-by-tag` | `file-path` `tag` `[namespace]` | `cr query defs -t`（项目范围） |
| `calcit.cli/validate-file` | `file-path` | `cr --check-only` |
| `calcit.cli/bump-version` | `file-path` `kind` | `cr config version` |
| `calcit.cli/edit-ns-doc` | `file-path` `namespace` `doc` | `cr edit ns-doc` |
| `calcit.cli/search-project` | `file-path` `pattern` `[filter]` … | `cr query search` |
| `calcit.cli/search-def-regex` | `file-path` `target` `regex` | `cr query search --regex`（单 def） |
| `calcit.cli/search-expr` | `file-path` `pattern` `[filter]` … | `cr query search-expr` |
| `calcit.cli/list-host-procs` | `[tag]` | `cr query host-procs` |
| `calcit.cli/analyze-check-types` | `file-path` `[namespace]` … | `cr analyze check-types` |
| `calcit.cli/analyze-weak-types` | `file-path` `[namespace]` … | `cr analyze weak-types` |
| `calcit.cli/docs-agents` | `[headings]` `[full]` | `cr docs agents` |
| `calcit.cli/docs-read` | `filename` `[headings]` … | `cr docs read` |
| `calcit.cli/docs-sections` | `filename` | `cr docs sections` |

类型分析逻辑已提取至 `src/type_coverage.rs`，供 `cr analyze` 与 `calcit.cli/analyze-*` 共用。

## 使用方式

### 基础用法

```bash
# heredoc 传入多行，内容完全不受 Shell 转义
cr project.cirru exec << 'END'
calcit.cli/list-ns $ {} (:file-path |project.cirru)
calcit.cli/list-defs $ {} (:file-path |project.cirru) (:namespace |app.core)
calcit.cli/peek-def $ {} (:file-path |project.cirru) (:target |app.core/main!) (:lines 8)
END

# 管道传入单行
echo 'calcit.cli/list-config $ {} (:file-path |project.cirru)' | cr project.cirru exec
```

### 写操作

```cirru.cli
; 创建新定义（需 Cirru one-liner 字符串）
calcit.cli/edit-def $ {} (:file-path |project.cirru) (:target |app.core/helper) (:code "|defn helper (x) (* x 2)") (:overwrite true)

; 替换 AST 节点
calcit.cli/tree-replace $ {} (:file-path |project.cirru) (:target |app.core/helper) (:path |2.0) (:code "|(+ x 1)")

; 搜索替换叶子
calcit.cli/search-replace $ {} (:file-path |project.cirru) (:target |app.core/helper) (:pattern |x) (:replacement |val)

; 添加 import
calcit.cli/add-import $ {} (:file-path |project.cirru) (:namespace |app.main) (:source-ns |app.core) (:refer-sym |helper)
```

## 约束与注意事项

### 执行模型

- `cr exec` 的 stdin 内容会被包装进 `app.main/main!`，多条顶层表达式按顺序执行，返回最后一个表达式的值。
- 若 stdin 全部是顶层定义，snippet 会被提升为多个定义，并自动补空的 `main!` / `reload!`。
- `cr exec` 不读取 stdin 之外的目标文件作为运行程序；第一个位置参数仍用于确定 base dir、模块解析和用户传给 `calcit.cli/*` 的路径示例。
- `calcit.cli/*` 函数操作的是显式传入的 `file-path`，不是隐式的当前 `cr exec` input。

### Core 声明约定（doc / schema / examples）

`calcit.cli/*` 采用与 `calcit.core/|~` 相同的**双源声明**模式：

| 来源 | 职责 |
| ---- | ---- |
| `src/bin/injection/mod.rs` | 运行时 handler 注册（单一 options map，arity 1） |
| `src/bin/injection/calcit_cli_specs.rs` | 各函数 map 选项键、类型、默认值 |
| `src/cirru/calcit-core.cirru` 的 `\|calcit.cli` | `:doc`、`:schema`、`:examples`、`:tags`；`:code $ quote &runtime-implementation` |

在 core 里维护类型的收益：

- `cr query schema calcit.cli/list-ns`、`cr query examples calcit.cli/edit-def` 可直接查阅
- 预处理与 `validate_def_vs_schema` 对非 `&runtime-implementation` 的 schema 做静态检查；host proc 的 code 占位符会被跳过
- Agent 文档与 ` ```cirru.cli ` 块可引用同一套 examples

同步方式：运行 `python3 scripts/gen_calcit_cli_core.py`，从 `mod.rs` + `calcit_cli_specs.rs` 生成/更新 `\|calcit.cli` 块（新增 Rust 注册后应重跑并补 examples）。

### 静态检查

`calcit.cli/*` 函数通过 `RegisteredProcDescriptor` 注册；调用时传入 `$ {}` 与 map 选项键，运行期由 `resolve_cli_args` 校验必填键、未知键与类型。

` ```cirru.cli ` 文档块用于展示这类调用。它应通过 `cr docs check-md` 的解析 + 预处理校验，但必须跳过执行，避免文档检查期间真实修改项目文件。

### 写操作约束

- `edit-def` 需传入 Cirru one-liner 字符串，用 `"|..."` 双引号包围以包含空格
- `tree-replace` 的 path 参数使用点号分隔（如 `3.1.0`），非法 path 必须报错，不能用 `filter_map` 静默丢弃非法段
- `add-import` 仅支持 `:refer` 导入格式，暂不支持 `:as` / `:rename` 等
- 写函数会直接修改 `.cirru` 文件，修改前不会备份；上层 Agent 仍需先查询、再局部编辑、再验证

### 设计风险

- `calcit.cli/*` 与传统 CLI 共享意图，但目前实现不一定共享同一套 handler，容易出现输出格式、搜索语义、import 合并规则不一致。
- 写函数如果绕过现有 `cli_handlers` 的校验与 diff 提示，会降低 `cr edit` / `cr tree` 已有安全性。
- `cr exec` 会执行任意 Calcit 代码，且 `calcit.cli/*` 具备文件写能力，默认只应作为本地开发工具使用。
- `cirru.cli` 文档块必须保证不会执行写操作；否则 `check-md` 会变成有副作用的文档测试。

### 未覆盖的 CLI 功能

以下功能尚无 `calcit.cli/*` 等价实现：

- `cr analyze check-types` / `weak-types` / `program-diff` — 类型覆盖分析逻辑仍在 `cr.rs`，待提取共享模块
- `cr docs` 结构化查询（`agents` / `read` / `sections` / `remote-libs` 等）— 可用 `docs-search` 或 `read-text-file` 部分替代
- `cr query error` / `cr --check-only` — 无直接等价（`show-error` 仅读 `.calcit-error.cirru`）

## 未来扩展

### 短期

- 优先让 `calcit.cli/*` 调用复用现有 `cli_handlers` 的核心逻辑，减少行为漂移
- 后续可为 `tree-show` 补齐 chunk 展示语义；MVP 先用 `max-lines` 控制输出规模
- 为写函数增加返回结构：是否修改、修改路径、建议验证命令

### 中期

- 将 `cr.rs` 中类型覆盖分析提取为共享模块，补全 `calcit.cli/analyze-check-types` 与 `analyze-weak-types`
- 补全 `calcit.cli/docs-*` 结构化查询（agents/read/sections）
- 为写函数增加返回结构：是否修改、修改路径、建议验证命令

### 长期

- MCP Server 模式：注册 `cr mcp` 子命令实现 Model Context Protocol，AI Agent 可直接通过 JSON-RPC 调用
- 可考虑将 `cr exec` 扩展为交互式 REPL 模式
