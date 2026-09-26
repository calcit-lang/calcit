# #1373：保留 Option lowering 之后的 payload 类型

## 根因与边界

公开 0.22.0 和 main 的 Manuscript `app.updater/updater` 均能复现：
`get` 从 `Map<String,Draft>` 读取之后，`match` 中的 `draft` 变成未绑定的 `T`，
导致 `assoc drafts pointer (assoc draft ...)` 被拒绝。

问题不在 `assoc` 的泛型替换。typed `get` 已经知道 `Option<Draft>`，但展开后的
`if (%some value) (%none)` 再次推断时，将没有 payload 的 `%none` 当作普通
`Option<Dynamic>` 分支，合并结果丢失了 `Draft`。因此不添加 `assoc` 特例，
也不让业务代码补 schema、改成 native call 或扩大 Dynamic。

只对已确认的零参数 core `%none` 构造表达式保留另一分支的 Option 类型参数。
普通未知 `Option<Dynamic>` 依然使合并结果变宽；不能据此假装取得静态证据。
复用已有 Result 构造分支识别，不增加公开语法、命令或分析器。

恢复精确类型后也暴露了同一路径的问题：有非空 guard 的 List `first`/`last`
使用 nullable primitive，给 payload 多加了一层 Optional。改用 guard 内的
`&list:nth`，保留列表元素本身的 nullable 类型与 `%some nil`。
String primitive 在公开契约中仍然 nullable；只在编译器已证明非空/索引存在的
生成分支内部，用会被擦除的 `assert-type :string` 保留该证据，不写回用户源码，
不添加 `unsafe-coerce`，不改变越界或非法索引的行为。

## 回归与验证

- `calcit.core/assoc` 的 definition `:tests` 覆盖 Struct 链式更新、写回 Map、
  Map `.get`/`.assoc` 方法及缺失 key。公开 0.22.0 运行新增用例失败，修复后通过。
- `calcit.core/first` 的 `:tests` 覆盖 List 首尾、空列表、真实 nil payload，
  以及 BMP String 的 first/last/nth/get、空串与越界。
- Rust 只补不能成功执行的错误诊断断言和内部未知分支合并边界；
  错误 Struct 字段和值仍应被拒绝。已有 lowering 测试保留“receiver 求值一次”断言。
- 将上述两个 `:tests` 的 AST 经 `query def --format json` 读出，用 `edit def`
  放入临时 Snapshot 的零参数入口，运行 native 和生成 JS；不维护第二份测试正文。
- Manuscript 不修改源码：严格检查、JS 生成通过；
  `query type-at app.updater/updater --path code@3.2.6.1.2.1.3.3 --format edn`
  返回精确的 `'app.schema/Draft`。
- 执行 `cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test`、`yarn check-all`（含 Agent interface、core tests、native/JS/WASM）。
  本地 HTTP fixture 需要沙箱外端口权限，不能把 PermissionDenied 当成代码缺陷。

## 不混入本次修复的发现

本地 Respo checkout 的 `collect-updating` 把 Dynamic key 传给 typed List `get`，
公开 0.22.0 与本分支均报相同 `E_ERASED_GENERIC_RELATION`，不是本次回归。
另外，Struct 的裸 `.assoc` 当前仍受 originless legacy impl 限制；本次保持既有
限制，不绕过 nominal trait 门禁。后续用实际 consumer 需求决定是否补齐该方法契约。

emoji 的 `last` 在公开 0.22.0 上 native 返回完整字符，而生成 JS 返回单个 surrogate；
独立缺陷已登记为 #1377，归入 0.23.0，不把替换字符当成正确预期。
