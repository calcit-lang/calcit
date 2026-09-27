# Result 成功值的 Dynamic 类型证据

## 问题

当本地参数的 schema 明确为 `Dynamic`，`resolve_type_value` 在没有额外 scope 记录时返回 `None`。直接泛型构造器 `%ok value` 因而无法代入 `T`，退回未绑定的 `Result<T,E>`；外层 `result:unwrap-or` 随后把仅用于错误分支的具体 fallback 绑定到 `T`，让 `query type-at` 把可能返回任意成功值的调用误报为精确 `Number`。

## 决策与边界

只在实参直接对应一个泛型变量、并且本地类型信息明确是 `Dynamic` 时保留这个开放边界，令返回类型代入 `Dynamic`。不把所有无法解析的表达式改成 Dynamic；例如参数要求 `List<T>` 而本地变量只有 Dynamic 时，仍无法证明它是列表，原有返回类型推断不得借此获得容器结构。普通 `%ok` / `Result :ok` 的动态成功值不因备用值被收窄；已证明没有成功 payload 的 core `:err` 仍按已合并规则从具体 fallback 推断，并保留错误类型。类型证据恢复后，移除 `core-result-method-v1` 对所有 `:ok` 构造的粗粒度禁止；具体 `%ok` 可在同一方法契约被证明时迁移，动态值仍保留审阅。这个修复不新增按函数名的类型特例。

## 验证

Calcit definition `:tests` 分别验证 `%ok` 和 `Result :ok` 的动态与具体成功值、同一泛型先收到 Number 后收到 Dynamic 并返回后者、以及原有 `:err` 行为。CLI 查询断言开放成功值为 `Dynamic` 且置信度 `unknown`，具体成功值和已知错误分支为精确 `Number`；严格检查中对开放值调用 `.add` 必须报 `E_DYNAMIC_POSTFIX_METHOD`，不能因 fallback 获得虚假的 Number 方法证据。Rust 单元测试覆盖泛型绑定（包含已有具体绑定被后续 Dynamic 扩宽）与复合参数不应获证的低层边界。全量门禁与 JS/native/WASM 回归分别验证受影响路径。
