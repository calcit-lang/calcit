# Nominal reflection and destruct APIs

## 修改概要

- 新增公开反射边界：`tuple-enum` 返回 `Option<Enum>`，`impl-origin` 返回 `Option<Trait>`，`record-struct` 返回确定存在的 `Struct`；底层 `&tuple:enum`、`&impl:origin` 保留为内部 nullable primitive，`&record:struct` 的签名修正为非空返回；
- 将 `destruct-list`、`destruct-map`、`destruct-set`、`destruct-str` 从匿名 `:: :some/:none` tuple 迁移为 `ListDestruct<T>`、`MapDestruct<K,V>`、`SetDestruct<T>`、`StringDestruct` 具名枚举，使分支和 payload 类型可由预处理器检查；
- 迁移核心测试、文档和示例到公开名义 API，并为 `record-struct` 补齐 WASM 公共包装层的内联 codegen；
- 收紧 legacy nil/tuple 诊断：只识别 canonical `calcit.core` 操作与 `calcit.core/Option`、`calcit.core/Result`，不再误报应用自定义的 `get`、`Option` 或枚举定义值；
- 修正 JS 边界一致性：`parse-float` 必须消费完整十进制字符串，`get-env` 缺失时返回 Calcit nil 而不是泄漏 JavaScript `undefined`；
- 收紧 native proc schema 回归测试，只允许带省略参数元数据的 proc 省略实参，并完整断言 nullable 返回类型。

## 知识点与兼容边界

- 反射 primitive 的 nil 是运行时表示细节，不应穿透公开 API；公开层必须把“可能不存在”提升为 `Option`，把运行时保证存在的记录来源声明为总函数；
- 集合解构不是单纯的可选值：成功分支还携带剩余集合，因此使用专用泛型枚举比嵌套 tuple 或 `Option<Tuple>` 更能保留 payload 关系；
- 名义迁移诊断必须同时限定操作来源和枚举完整路径；仅按符号名匹配会污染用户空间，并曾导致 tuples 文档把 `Result` 枚举定义本身误判为旧式值；
- WASM 当前可直接内联 `record-struct`；`tuple-enum` 和 `impl-origin` 仍受 WASM 后端尚未保留 enum/trait 反射元数据的既有边界限制；
- core 弱类型审计目前剩 16 个 code-nil：15 个未解析位置主要属于宏 AST sentinel/自举兼容，1 个是 `last` 的遗留 `Optional`。`first`/`last`/`nth`/`get` 的下一阶段迁移需要连同 method inliner 与 core bootstrap 一起设计，不能机械替换。

## 验证

- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`（349 lib + 2 caps + 180 cr）
- `yarn compile`
- `CR_WASM_BIN=./target/debug/cr-wasm yarn check-all`（Native、JS、IR、Agent interface、WASM 全通过）
- 7 份变更文档通过 `cr docs check-md --entry calcit/test.cirru`
- 新公开 API 与四个 destruct 枚举的定向 examples 和静态类型断言通过
- 安装当前全局 `cr` 后只读检查 Respo：294 个定义完成类型覆盖分析，`respo.util.format/get-style-value` 的 3 个示例通过；诊断能发现该项目现存 legacy Optional 与不安全 JsNullish 解引用。
