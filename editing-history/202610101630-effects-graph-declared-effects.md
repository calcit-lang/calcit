# effects-graph 只按声明分类效果

关联 #1567、#1457、#1566。`analyze effects-graph` 原先用 `classify_by_name` 与 `heuristic_effect_kinds` 按名字猜测：名字含 `load`/`init`/`setup` 记为 `io`，以 `!` 结尾记为 `effect`，Respo 风格的 `render!`、`dispatch!` 等有专用规则。仓库 `calcit/*.cirru` 的实测清单中，`data-definition-form`、`payload-form`、`load-data-code` 等纯定义因此被记为 `io`，而没有 `!` 的旧 `.cancel` 完全没有被识别。

## 决策

效果只来自四处声明：core 定义的 Snapshot `:tags`（没有 Snapshot entry 的 builtin proc/syntax 在 `src/effects_graph.rs` 逐个列出，测试保证每个 proc/syntax 恰好有一处声明）、宿主 proc 的 descriptor tags、core `defimpl` / `&impl::new` 方法表指向的实现定义 tags，以及调用图子节点。找不到声明时输出 `unknown`：未加载的限定名、未声明 tags 的宿主 proc、项目或模块也实现了的同名方法。裸符号无法解析时按局部绑定/函数值处理，不分类。

按 `docs/features/api-roles.md` 的 `!` 含义（显式写入、生命周期、注册与取消）补 core tags：`ffi-task:cancel`、`ffi-task:cancel-with`、`ffi-response:resolve`、`ffi-response:reject`、`&init-builtin-impls!`、`&reset-gensym-index!` 加 `:effect`；`non-nil!` 加 `:control`；`try-read-dir` 加 `:file :io`；`dbg` 加 `:log`。`hint-fn` 只有在 hint schema 出现 `:async` 时才记为 `async`，普通类型提示不再算作异步。

同时让 `alias/def` 通过 `:as` 导入解析为调用图子节点，JS 字符串模块导入归为 `interop/js`，`--format json` 的起始提示改写到 stderr，使 stdout 只有一个 JSON 文档。

## 兼容

效果 kind 的拼写随来源统一：`io/read`/`io/write` 合并为 `io/file`，`control/raise`/`control/quit` 为 `control`，`interop/eval` 为 `interop/host`，`effect/sequential` 为 `effect`，`render`/`lifecycle`/`storage` 等名字规则产生的 kind 不再出现。`effects-graph` 仍是只读、非阻断报告，不进入函数类型。calcit.std 与 js-ffi 的模块定义 tags 尚未被读入，需要另行在模块仓库与加载链路中处理。

## 验证

`cargo test --lib effects_graph` 覆盖 tag 分类、方法表、名字不再决定效果、宿主 descriptor 与 hidden proc 声明完整性，以及 `calcit/test-effects-graph.cirru` 的端到端结果。对 `calcit/*.cirru` 全部 fixture 运行新旧二进制并比较效果清单。
