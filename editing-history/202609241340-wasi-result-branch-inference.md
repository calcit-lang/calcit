# WASI 清单业务的 Result 分支类型合并

## 问题

`app.main/transform-manifest` 的两个 `if` 分支分别直接构造 `%err String` 与 `%ok Manifest`。此前 `query type-at ... --path @3` 将整个表达式报告为 `Result<Dynamic, String>`（partial），尽管成功分支已有具名 `Manifest`。业务函数因此仍需要显式返回 schema 来保护后续方法调用。

## 改动

仅当两个分支确实分别是核心 `%ok` 和 `%err` 构造器，且各自未指定的另一个泛型槽仍为 `Dynamic` 时，组合已知的 payload 类型。普通 `if`、同种构造器、任意显式动态 payload 和其他泛型合并路径保持原样；不引入新的检测器或动态类型阈值。

同一 Calcit 业务源码保持不变：`decode-manifest` 是文本进入具名结构的唯一显式解码边界；`process-manifest` 继续使用普通 `.and-then` / `.map` 方法，没有新增 `assert-type`、`unsafe-coerce` 或 native call。此切片消除的是编译器查询结果中不必要的开放槽，而不是声称已经删掉一项源码标注。返回 schema 暂时保留为公开契约；后续再审阅是否可安全移除。

## 验收证据

- `transform-manifest` 的 `@3`：此前 `Result<Dynamic, String>`（partial），现在 `Result<Manifest, String>`（exact）。
- `process-manifest` 的 `@3`：`Result<String, String>`（exact）；闭包绑定 `manifest`、`updated` 均为具名 `Manifest`，`.and-then` 降为静态方法调用。
- Rust 单元测试限定互补构造器的合并条件；CLI 集成测试从真实 Snapshot 查询具名 payload。
- Manifest `:tests` 与 native / Node JS / Preview 1 / 真实 WASI 0.3 Component 的业务回归由同一 fixture 驱动。

## 有副作用方法实参的后续复现

增加独立的 `method-eval-main!` 验收入口，仍调用普通 `Result.map`：receiver 在 `do` 中输出 `receiver`，回调实参在另一个 `do` 中输出 `argument`，回调结果输出 `api!`。native 路径原本可执行，但 WASI 0.3 Component 报“nested fn/defn closure values are not yet supported”，尽管 `query type-at` 已显示精确 `Result<String, String>`、具名 `Manifest` 回调参数与静态方法 lowering。

WASM 静态调用专化现在识别仅由无绑定 `do`/`CoreLet` 顺序表达式包住的内联回调，按实参从左到右先执行副作用，再捕获并内联回调；不把任意闭包值开放给 WASM，也不提前执行回调体。业务脚本比较 native、Node JS 和真实 WASI 0.3 Component 的相同三行标记，证明 receiver 与参数各求值一次且顺序相同。

负例测试使用 Calcit CLI 在临时 Snapshot 中把成功分支的 `Manifest` payload 替换为 `String`；严格预处理以 `W_FN_RETURN_TYPE_MISMATCH` 拒绝 `Result<String, String>` 冒充 `Result<Manifest, String>`，没有静默扩宽。

剩余边界：Cirru EDN 输入仍需在 `try-parse-cirru-edn-as` 一次性验证为 `Manifest`。本次没有扩大任意 `Dynamic` 值的推断权限。
