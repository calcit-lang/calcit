# Public nil contract freeze

## 修改概要

- 将公开 `first`、`last`、`nth`、`get`、`get-in` 及其方法入口统一为名义 `Option<T>`，同时把 core 自举和已证明安全的内部路径迁到 `&list:*`、`&map:*`、`&record:*` raw primitive；
- 修复公开方法表泄漏：Map/Set `.destruct` 返回具名 `MapDestruct`/`SetDestruct`；Record 不再暴露跨后端顺序不稳定的 `.nth`，字段读取统一走返回 `Option<T>` 的 `get`；
- 将 `when-let` 固定为 `Option<T> -> Option<R>`，将 `update-in` updater 固定为 `Option<T> -> T`，缺失叶子不再通过 nil 传递；
- 非穷尽 `case` 改为明确报错，`cond` 要求最终 `true` 分支；`dissoc-in` 空路径改为保持输入值；
- 公开 core 与应用定义若暴露 legacy `Optional<T>` 会触发 `W_LEGACY_OPTIONAL_SCHEMA`，仅 `&` raw primitive 与 `optionally` 桥接豁免；
- `;nil` 明确定义为 Unit marker；弱类型审计把 Unit 定义内的 nil 归为 `declared-unit`，core 的 unresolved/declared-optional code-nil 门禁归零；
- 更新原生/JS/WASM 测试、类型推断用例、RFC 和迁移文档，集中声明下一版本是 nil 迁移的最终 breaking window。
- 保留 JS runtime 的 legacy `get_env` 导出并委托给内部 `_$n_get_env`，使升级 npm runtime 时尚未重新生成的 JS bundle 仍可启动；新生成源码仍只能通过 typed `get-env -> Option<String>` 或内部 raw proc，不把兼容别名重新暴露为 Calcit API。

## 知识点与兼容边界

- raw primitive 可以保留 nullable ABI，但不得直接挂入公开 method table；方法表本身是公开 API 审计的一部分；
- 类型系统只能检查重新预处理的 Calcit 源码，无法检查用户手中已经生成的旧 JS bundle；因此底层 JS export 改名还需要一层薄兼容别名，避免“旧 bundle + 新 npm runtime”直接在模块加载阶段崩溃；
- 静态已知 Record 字段 tag access 是总操作，可由编译器优化为内部 `&record:nth`；公开 API 不承诺字段位置顺序，只提供带 `Option` 外层的字段名 `get`；
- partial record 的字段名属于结构，但 payload 仍可能是 nil，因此合法结果可以是 `%some nil`；`%none` 只表示键/索引不存在；
- `update-in` 的缺失与“存在且值为 nil”必须由 `%none` / `%some nil` 区分，updater 不再猜测裸 nil 的含义；
- `Unit` 仍由运行时 nil 承载，但源码使用 `;nil` 只表达无业务值，不能作为 Option 的替代品；
- 下一版本发布后，新增缺失 API 必须首次发布即采用 Option/Result/Unit。以后不得再以清理 nil 为由改变同一公开返回类型。

## 验证清单

- `cargo fmt --all -- --check`：通过；
- `cargo clippy -- -D warnings`：通过；
- `cargo test`：通过（lib 349、caps 2、cr 180，另含 doc tests）；
- `yarn compile`：通过；
- `yarn check-all`：通过，覆盖 Native、JS、IR、WASM 与 Agent interface；
- `yarn check-agent-interface`：12/12 通过；
- core `analyze weak-types --only code-nil --intent unresolved,declared-optional`：零结果；
- 变更涉及的 Markdown examples：全部通过 `docs check-md`；
- 全局安装当前工作树构建的 `cr` 后，对 Respo 核心库执行只读回归：`--check-only` 在旧代码把 `first` 的 `Option` 结果继续交给 `&list:nth` 时立即失败，证明升级断点可发现；`analyze weak-types --only code-nil --intent unresolved,declared-optional --summary-only` 报告 57 处待迁移 nil（54 unresolved、3 declared-optional），为下游一次性迁移提供清单。该回归不修改 Respo。
- 在临时 Respo 副本修正宏展开的首个 Option 断点后，`--check-only` 继续以 60 条诊断阻断旧源码，其中 17 条 `W_JS_FFI_NULLABLE_DEREF`、5 条 `W_FN_ARG_TYPE_MISMATCH`、8 条 `W_FN_RETURN_TYPE_MISMATCH`；覆盖 `if-let` 误收 JsNullish、未收窄的 DOM 解引用、FFI 返回冒充 Unit、`Date.now` 未验证即参与数值运算等真实边界。
- 用 Respo 已生成的浏览器 bundle 强制链接当前工作树 `lib/calcit.procs.mjs`：首次加载发现缺失 legacy `get_env` export；补兼容别名后，干净页面加载无 console error/warning，并成功输入、添加一条 Todo，证明 DOM 渲染与事件 FFI 路径可运行。整个过程只修改 `/private/tmp` 副本，不修改 Respo 原仓库。
