# Expand nominal Option collection APIs

## 修改概要

- 将 `&list:find-last`、`&list:find-last-index`、`&list:last-index-of` 的缺失结果迁移为 `Option`；
- 将 List/Set 的 `max`、`min` 迁移为 `Option<Number>`，空集合明确返回 `%none`；
- 保留内部 `&str:find-index` 的数值哨兵 ABI，新增公开 `str-find-index: String,String -> Option<Number>`，字符串 `.find-index` 指向公开 API；
- 保留内部 `&get-in: Dynamic,List<K> -> Optional<Dynamic>`，公开 `get-in` 通过 `optionally` 返回 `Option<Dynamic>`，字面量路径推断继续保留精确 payload；
- 修正 WASM 针对 find/find-index/max/min 的优化发射路径，确保它们与 Native、JS 一样构造 `%some`/`%none` 名义 tuple；
- 扩展 `W_NOMINAL_ENUM_LEGACY_USE`：拦截 Option/Result 与 payload 混合比较、payload 类型谓词、tuple/collection 表示操作和直接 truthiness；同一名义枚举之间的相等比较仍然允许；
- 更新测试与文档中的旧 nullable 用法。

## 知识点

- 公开 API 应返回名义 `Option`，但内部递归、宿主 ABI 和底层哨兵可以继续使用 `Optional`/数值哨兵；两层接口要用命名和 `:internal` 标签清楚分隔；
- JS FFI 的 `Optional<JsObject>` 不能沿用普通数据 API 的 Option 包装策略。它同时表达宿主空值和未验证对象，必须由专门的 nullable dereference 与返回类型诊断负责；
- `%none` 本身是 truthy tuple。迁移诊断不能只检查 `nil?`/`some?`，还要检查 `if option`、`record? option`、`count option`、Option 与 payload 的 `=`，否则 breaking change 可能静默改变控制流；
- 相等比较的告警需要检查所有参数：`Option == Option` 合法，`Option == payload` 或 `Option == Dynamic` 才是迁移风险；
- WASM 的专用优化 emitter 会绕过普通 core 函数体。只改 Native/JS/runtime 定义不足以改变 WASM 语义，名义返回值必须在 emitter 中显式分配 enum tuple；
- `cr-wasm` 会嵌入构建时的 core Snapshot。验证未提交 core 改动时应重新构建当前二进制，并通过 `CR_WASM_BIN` 明确选择它，避免旧 release 产物造成假失败。

## 验证

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`（346 lib + 2 caps + 180 cr）
- `yarn compile`
- `CR_WASM_BIN=./target/debug/cr-wasm yarn check-all`（Native、JS、IR、WASM 全通过）
- `yarn check-agent-interface`（12/12）
- 四份变更文档的 `cr docs check-md`（58/58 blocks）
- 安装当前全局 `cr` 后只读检查 Respo：4 条 Option 迁移、1 条旧 `get-env` arity、10 条 JS FFI nullable dereference、6 条 JS/Unit 返回不匹配；JS FFI 告警未被 Option 迁移吞掉，Respo 工作区保持干净。
