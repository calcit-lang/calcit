# Separate JavaScript nullish boundaries

## 修改概要

- 新增 `JsNullish<T>` 类型标注，专门表达 JavaScript FFI 的 `null`/`undefined` 边界，并补齐解析、序列化、泛型替换、类型覆盖和诊断展示；
- 将 raw `js/...`、预处理后的 `RawCode`、原生属性读取与方法调用、`aget`/`js-get` 的保守返回类型改为 `JsNullish<JsObject>`；
- 明确禁止 `JsNullish<T>` 与遗留 `Optional<T>` 相互匹配，避免通用 `optionally` 把宿主空值静默包装为 `Option`；
- 新增 `js-nullish?`、`js-present?` 和显式 `js-nullish->option`，并为旧 `nil?`/`some?` 判空、不安全 FFI 解引用提供专用诊断与分支收窄；
- 对非 core 函数 schema 中的 `Optional<T>` 发出阻断性 `W_LEGACY_OPTIONAL_SCHEMA`，普通缺失改用 `Option`，失败改用 `Result`，副作用返回改用 `Unit`；core 自举依赖的旧 nullable API 暂时保留内部兼容表示；
- 更新 JS 集成用例、静态分析文档与 nil reduction RFC，不引入 `Nilable`。

## 知识点

- `js/...` 符号在预处理后会变成 `Calcit::RawCode`；只在符号推断分支处理 FFI 会漏掉真实项目，因此 standalone RawCode 与 RawCode call head 都必须保留 `JsNullish<JsObject>`；
- `JsNullish<T>` 是宿主边界的 union-like 标注，不是 Calcit 业务数据。判空只证明 payload 存在，不能把 opaque `JsObject` 自动证明为 `Number`、`String` 或记录；可信契约必须显式 `unsafe-coerce`，普通代码优先使用 decoder；
- Optional 与 JsNullish 的双向不匹配是迁移安全约束：即使两者运行时都可能由 nil 承载，类型系统也不能让 JS 空值沿旧兼容 API 静默进入名义 Option；
- 全局 `document`、`localStorage`、`Math`、`Date` 等也按保守 FFI 边界处理。可选原生访问适合真实可空路径，已知宿主契约则在最小边界处显式收窄或转换；
- 公开 Optional 先采用可聚合的阻断 warning，而不是首条即退出的 schema error，这样 Respo 等大项目一次检查即可列出全部迁移点。

## 验证

- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`（349 lib + 2 caps + 180 cr）
- `yarn compile`
- `CR_WASM_BIN=./target/debug/cr-wasm yarn check-all`（Native、JS、Agent interface、WASM 全通过）
- `yarn check-agent-interface`（12/12）
- 三份变更文档的 `cr docs check-md`（64/64 blocks）
- 安装当前全局 `cr` 后只读检查 Respo：共 55 条迁移诊断，其中 18 条不安全 JS 解引用、4 条旧 JS 判空、16 条公开 Optional schema、11 条函数参数/返回类型不匹配、1 条底层 proc 参数不匹配、4 条名义枚举旧用法、1 条 `get-env` arity；Respo 工作树保持干净。
