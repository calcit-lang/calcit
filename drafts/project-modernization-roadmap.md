# 项目现代化路线图（Rust 导向）

## 为什么现在做

当前架构已可用，但仍存在一些在现代 Rust 项目中通常会优先清理的可维护性热点：

- `src/` 下模块边界较宽，部分职责混合了 runtime/codegen/CLI 关注点；
- 仍有遗留/重复文件（例如：`src/calcit/struct.rs` 与 `src/calcit/calcit_struct.rs`）；
- 生成产物流程依赖本地命令与脚本，尚未稳定地固化为 CI/任务体系；
- 基准与验证流程主要靠“口口相传”的命令知识，不是项目内的一等资产。

## 阶段 1：低风险整理（高 ROI）

### 1）移除无效/重复模块

- 候选：`src/calcit/struct.rs` 看起来是遗留文件，且未被 `src/calcit.rs` 接入。
- 保留 `src/calcit/calcit_struct.rs` 作为唯一权威实现。

预期收益：

- 降低核心类型导航时的认知负担；
- 降低“改到错误文件”的风险。

### 2）统一模块命名约定

当前风格混用了 `calcit_struct.rs` 与 `list.rs`/`tuple.rs` 这类通用命名。

建议：

- 在 `src/calcit/` 选择并坚持一种命名约定：
  - 要么全部采用 `calcit_*.rs`；
  - 要么全部采用类型语义命名，并由统一索引模块导出。

预期收益：

- `grep` 与跳转更直接；
- 对新贡献者与工具链更一致友好。

### 3）把高频检查提升为脚本/任务

将关键验证收敛为显式脚本（或任务别名），并保证输出稳定：

- 格式化/静态检查（`cargo fmt`、`cargo clippy -- -D warnings`）；
- 编译/测试（`yarn compile`、`cargo test`、`yarn check-all`）；
- 聚焦 JS 路径的 benchmark smoke test。

预期收益：

- 减少命令漂移；
- 本地开发与 CI 更容易对齐。

## 阶段 2：结构化整理（中等工作量）

### 4）将 `src/codegen/emit_js.rs` 拆为内聚子模块

`emit_js.rs` 体量较大，当前承载了多类职责：

- 符号转义；
- 参数/recur 模板决策；
- import/tag 组织；
- 表达式渲染。

建议：

- 按职责拆为子模块，例如：
  - `emit_js/symbols.rs`
  - `emit_js/functions.rs`
  - `emit_js/imports.rs`
  - `emit_js/tags.rs`

预期收益：

- 评审可读性更好；
- 后续优化时回归风险更低。

### 5）收敛 runtime helper 接口面

随着 helper 持续增加（`_args_throw`、`_args_fewer_throw`、`_args_between_throw`、`init_tags`），建议在 TS runtime 内按能力分组并形成明确边界：

- `arity helpers`；
- `tag helpers`；
- `list/map helpers` 等。

预期收益：

- JS codegen 演进更平滑；
- API 兼容边界更清晰。

## 阶段 3：现代 Rust 项目形态（较大工作量）

### 6）评估 Cargo workspace 拆分

可考虑的内部 crate：

- `calcit-core`（数据模型 + 求值器）；
- `calcit-codegen-js`；
- `calcit-cli`。

预期收益：

- 编译影响域更小；
- 对外/对内 API 边界更清楚；
- 长期模块化能力更强。

### 7）为热点路径引入 criterion 基准

将以下基准固化为可重复执行资产：

- tail recursion 模板路径；
- tag 初始化开销；
- rest 参数转换路径。

预期收益：

- 优化决策更客观；
- 避免凭体感调优。

## 建议执行顺序

1. 先做 dead file 清理 + 命名规范收敛。
2. 再做 JS codegen 文件拆分（仅重组，不改行为）。
3. 引入 benchmark harness。
4. 边界稳定后，再评估 workspace 拆分。

## 执行追踪计划（用于持续跟踪）

> 状态约定：`TODO` / `DOING` / `DONE`

### Milestone A（稳健起步，低风险）

- [x] `DONE` 把路线图转为中文并统一结构。
- [x] `DONE` 清理 dead/重复模块（已移除：`src/calcit/struct.rs`）。
- [x] `DONE` 在关键位置补测试（覆盖 JS codegen 新模板行为）：
  - `tmpl_import_procs` 是否引入 `init_tags`；
  - `tmpl_tags_init` 是否统一走 `init_tags(...)`；
  - `tmpl_tail_recursion` 是否包含周期 watchdog 语义。
- [x] `DONE` 跑针对性测试 + 全量回归（`cargo test -q snippets::tests` / `yarn check-all`）。
- [x] `DONE` 将高频验证命令固化为脚本（`fmt-rs`/`lint-rs`/`test-rs`/`test-snippets`/`bench-recur-smoke`/`check-smooth`）。

验收标准：

- dead file 不再保留重复定义；
- 新增测试能锁定关键模板行为，避免回归；
- 所有验证命令通过。

### Milestone B（结构化整理）

- [ ] `DOING` 拆分 `emit_js.rs`（仅移动代码，不改变行为）。
  - 已完成子任务：抽离 `tag_access` / `is_simple_tag_name` 到 `src/codegen/emit_js/tags.rs`，并补充对应单测。
  - 已完成子任务：抽离 `escape_var` / `escape_cirru_str` 到 `src/codegen/emit_js/symbols.rs`，并补充对应单测。
  - 已完成子任务：抽离 `to_js_import_name` / `to_mjs_filename` 到 `src/codegen/emit_js/paths.rs`，并补充对应单测。
  - 已完成子任务：抽离 `get_proc_prefix` / `is_cirru_string` 到 `src/codegen/emit_js/runtime.rs`，并补充对应单测。
  - 已完成子任务：抽离 `gen_args_code` 到 `src/codegen/emit_js/args.rs`，并补充普通参数/展开参数单测。
  - 已完成子任务：抽离 `gen_call_args_with_temps` 及其内部判定 helper 到 `src/codegen/emit_js/args.rs`。
  - 已完成子任务：抽离 `contains_symbol` / `sort_by_deps` 到 `src/codegen/emit_js/deps.rs`，并补充依赖判定与排序稳定性单测。
  - 已完成子任务：抽离 `write_file_if_changed` / `is_js_unavailable_procs` / `cirru_to_js` 到 `src/codegen/emit_js/helpers.rs`，并补充基础单测。
  - 当前策略：每次只做一个低风险子模块迁移，迁移后立即执行定向测试 + `yarn check-all`。
  - 下一批拆分清单（按顺序执行）：
    - [x] 抽离 `get_proc_prefix` / `is_cirru_string` 到 `src/codegen/emit_js/runtime.rs`。
    - [x] 评估并抽离参数拼接相关 helper（已完成 `gen_args_code` + `gen_call_args_with_temps`）。
    - [x] 评估并抽离 `contains_symbol` + 依赖排序工具（若仅内部使用则独立 `deps.rs`）。
    - [x] 抽离通用 helper（`write_file_if_changed` / `is_js_unavailable_procs` / `cirru_to_js`）。
- [x] `DONE` 收敛 runtime helper 分组与导出边界。
  - [x] 已完成：将 arity helper（`_args_throw` / `_args_fewer_throw` / `_args_between_throw`）抽离到 `ts-src/js-arity-helpers.mts`。
  - [x] 已完成：将 tag helper（`init_tags`）抽离到 `ts-src/js-tag-helpers.mts`。
  - [x] 已完成：通过 `calcit.procs.mts` 统一 re-export，保持对外 API 名称不变。
  - [x] 已完成：补充 runtime helper 分组说明文档（最小化一段说明，记录导出边界）。
  - 分组边界说明：
    - `js-arity-helpers.mts` 仅承载参数个数校验错误构造（返回 `Error`，不包含业务逻辑）。
    - `js-tag-helpers.mts` 仅承载 tag 初始化与缓存（`init_tags` + 内部 `_tag_cache`）。
    - `calcit.procs.mts` 作为聚合入口，负责稳定导出名，不承载上述 helper 的实现细节。

验收标准：

- 模块文件体量下降、职责更聚焦；
- 无行为变化，测试与 `check-all` 全通过。

### Milestone C（工程化现代化）

- [ ] `TODO` 引入 benchmark harness（优先 tail recursion/tag/rest 路径）。
- [ ] `TODO` 评估并设计 workspace 拆分方案（先设计后实施）。

验收标准：

- 基准可重复执行并有基线数据；
- workspace 拆分有明确边界与迁移顺序。

## AI 协作导向的深层规划（在关键语义不动摇前提下）

目标约束（保持不变）：

- snapshot / 不可变数据模型；
- trait 体系与 impl 优先级语义；
- 宏展开能力与 hygienic 机制；
- JS 编译目标与运行时兼容边界。

核心方向（为 AI “错误左移 + 用法发现”服务）：

1. **把错误变成契约**：
  - 将“预期失败/预期告警”场景转为可重复检查（负向测试契约），避免回归时 AI 只能依赖模糊日志。
2. **把契约变成分层入口**：
  - 保留 `test.cirru` 作为主路径正向回归；
  - 新增独立入口的“shift-left 契约检查”脚本，用于验证错误信息与阻断行为是否稳定。
3. **把分歧变成可定位信号**：
  - 优先统一错误文本的结构（场景 + 期望 + 实际 + 定位），降低 AI 修复时的歧义。

近期执行（已落地/可直接使用）：

- [x] 新增 `yarn check-shift-left`（`scripts/check-shift-left.mjs`），覆盖：
  - 正向基线：`calcit/test.cirru -1` 必须通过；
  - 负向契约：`test-proc-type-warnings` / `test-method-validation` / `test-ir-type-info` 必须失败且命中关键诊断文本。

下一步建议（优先级顺序）：

- [ ] 给 `check-shift-left` 再补 2~3 个稳定负向用例（例如 tag-match/arity 相关），先修复不稳定输入文件再纳入。
- [ ] 梳理一份“诊断文案规范表”（错误类别、必须字段、示例）并用于后续错误信息统一。
- [ ] 在 guidebook 补一节 “AI 友好排错路径”，把常见失败 -> 命令 -> 预期输出串成最短闭环。
