# effects-graph 只按声明分类效果

关联 #1567、#1457、#1566。`analyze effects-graph` 原先用 `classify_by_name` 与 `heuristic_effect_kinds` 按名字猜测：名字含 `load`/`init`/`setup` 记为 `io`，以 `!` 结尾记为 `effect`，Respo 风格的 `render!`、`dispatch!` 等有专用规则。仓库 `calcit/*.cirru` 的实测清单中，`data-definition-form`、`payload-form`、`load-data-code` 等纯定义因此被记为 `io`，而没有 `!` 的旧 `.cancel` 完全没有被识别。

## 决策

效果只来自四处声明：core 定义的 Snapshot `:tags`（没有 Snapshot entry 的 builtin proc/syntax 在 `src/effects_graph.rs` 逐个列出，测试保证每个 proc/syntax 恰好有一处声明）、宿主 proc 的 descriptor tags、core `defimpl` / `&impl::new` 方法表指向的实现定义 tags，以及调用图子节点。方法表中内联的实现（`&core-enum-methods` / `&core-struct-methods` 的 `defn ...-impl`）沿用所在 core 定义的 tags。找不到声明时输出 `unknown`：未加载的限定名、未声明 tags 的宿主 proc、项目或模块也实现了的同名方法，以及调用所在 `defn` / `defmacro` / `fn` 的参数（调用方传入的函数值）。参数按 `defn` / `defmacro` / `fn` 逐层入栈出栈，只在所在函数内有效；`let` / `loop` / `&let` / `if-let` / `when-let` / `&doseq` / `doseq` / `let[]` / `let{}` 绑定的同名局部遮蔽参数。为避免把非调用当作调用，参数列表、上述绑定对与解构名，以及 `case` / `match` / `tag-match` / `list-match` / `struct-match` / `cond` 分支和 `case-default` 第 3 项起的分支不检查头部（`case-default` 的默认值按表达式分析），`(receiver .method args)` 按方法调用处理。其余无法解析的裸符号（如 `let` 绑定的函数值）不分类。

按 `docs/features/api-roles.md` 的 `!` 含义（显式写入、生命周期、注册与取消）补 core tags：`ffi-task:cancel`、`ffi-task:cancel-with`、`ffi-response:resolve`、`ffi-response:reject`、`&init-builtin-impls!`、`&reset-gensym-index!` 加 `:effect`；`non-nil!` 加 `:control`；`try-read-dir` 加 `:file :io`；`dbg` 加 `:log`（保留 `:macro`）。`hint-fn` 复用 `CalcitTypeAnnotation::hint_form_marks_async`（JS lowering 的同一判断）：只有所在函数自身的单参数 schema 写 `:async true` 才记为 `async`；`(:async false)`、普通类型提示、`:args` 中的异步回调类型（如 core `accept-async`）与带目标的双参数 `hint-fn` 都不算异步。`.!name` / `.?!name` 在输出中保留原前缀。

同时让 `alias/def` 通过 `:as` 导入解析为调用图子节点，JS 字符串模块导入归为 `interop/js`，`--format json` 的起始提示改写到 stderr，使 stdout 只有一个 JSON 文档。

## 兼容

效果 kind 的拼写随来源统一：`io/read`/`io/write` 合并为 `io/file`，`control/raise`/`control/quit` 为 `control`，`interop/eval` 为 `interop/host`，`effect/sequential` 为 `effect`，`render`/`lifecycle`/`storage` 等名字规则产生的 kind 不再出现。`effects-graph` 仍是只读、非阻断报告，不进入函数类型。calcit.std 与 js-ffi 的模块定义 tags 尚未被读入，需要另行在模块仓库与加载链路中处理。

## 验证

`cargo test --lib effects_graph` 覆盖 tag 分类、方法表、名字不再决定效果、宿主 descriptor 与 hidden proc 声明完整性，以及 `calcit/test-effects-graph.cirru` 的端到端结果。对 `calcit/*.cirru` 全部 fixture 运行新旧二进制并比较效果清单。
