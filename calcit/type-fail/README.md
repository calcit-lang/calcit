# type-fail fixtures (schema)

这个目录存放**预期在 preprocess 阶段失败**的示例，用来验证最近增加的 `:schema` 类型描述能正确生效。

## 运行方式

在项目根目录执行（都会返回非 0）:

- `cargo run --bin calcit -- calcit/type-fail/schema-required-arity.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-rest-missing.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-rest-unexpected.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-kind-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/schema-call-arg-type-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/trait-method-generic-receiver-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/generic-where-bound-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/slice-receiver-trait-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/update-collection-contract-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/collection-member-contract-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-record-call-arg-type-mismatch.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-bind-unknown.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-bind-duplicate.cirru --check-only`
- `cargo run --bin calcit -- calcit/type-fail/type-slot-unbound-strict.cirru --strict-types --check-only`
- `cargo run --bin calcit -- calcit/type-fail/whole-dynamic-schema-strict.cirru --strict-types --check-only`
- `cargo run --bin calcit -- calcit/type-fail/dynamic-nominal-method-strict.cirru --strict-types --check-only`
- `cargo run --bin calcit -- calcit/type-fail/dynamic-method-dispatch-strict.cirru --strict-types --check-only`
- `cargo run --bin calcit -- calcit/type-fail/raw-primitive-strict.cirru --strict-types --check-only`
- `cargo run --bin calcit -- calcit/type-fail/erased-generic-relation-strict.cirru --strict-types --check-only`

其中：

- 前 4 个会触发 `schema mismatch while preprocessing definition`（定义时校验）。
- `schema-call-arg-type-mismatch.cirru` 会触发基于 schema 的函数参数类型告警，并在 `--check-only` 下被当作错误处理。
- `trait-method-generic-receiver-mismatch.cirru` 会验证泛型方法根据 receiver 的 `Option<String>` 绑定其 fallback 类型，并拒绝 `Number` fallback；同时验证 `.and-then` callback 不能返回裸 payload。
- `generic-where-bound-mismatch.cirru` 会触发 `W_GENERIC_WHERE_BOUND_MISMATCH`，验证泛型 `:where` 约束在调用点能被发现，并在 `--check-only` 下被当作错误处理。
- `slice-receiver-trait-mismatch.cirru` 会验证 `slice` 只接受实现 `Sliceable` 的 receiver，拒绝用 `C -> C` 泛型伪装不可切片的值。
- `update-collection-contract-mismatch.cirru` 会验证已知 `List<T>` / `Map<K,V>` receiver 将索引/键与 `T -> T` / `V -> V` updater contract 带到调用点。
- `collection-member-contract-mismatch.cirru` 会验证已知 collection receiver 将 `get` / `contains?` 的索引或键、`includes?` 的成员、`assoc` 的索引/键/值，以及 `dissoc` 的所有 rest 索引或键类型带到调用点，包括 Enum 的 Number payload index；它也覆盖原生 `&map:dissoc`、`&list:concat` 与 `&merge` 的同质 variadic contract。
- `type-slot-record-call-arg-type-mismatch.cirru` 会验证 `bind-type` 绑定 struct 实例后，`*slot` 参与调用点类型检查。
- `type-slot-bind-unknown.cirru` 会验证未声明 slot 的 `bind-type` 会直接失败。
- `type-slot-bind-duplicate.cirru` 会验证同一个 slot 重复绑定会直接失败。
- `type-slot-unbound-strict.cirru` 会验证 strict 模式拒绝可达定义中的未绑定 slot，并报告稳定的 `E_UNBOUND_TYPE_SLOT`。
- `whole-dynamic-schema-strict.cirru` 会验证 strict 模式拒绝可达定义的 whole-Dynamic function contract，并报告稳定的 `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA`。
- `dynamic-nominal-method-strict.cirru` 会验证 strict 模式拒绝 Dynamic receiver 上的 Option/Result nominal method dispatch，并报告稳定的 `E_DYNAMIC_POSTFIX_METHOD`。
- `dynamic-method-dispatch-strict.cirru` 会验证 strict 模式拒绝普通 Dynamic receiver method，并报告 receiver source 与稳定的 `E_DYNAMIC_POSTFIX_METHOD`。
- `raw-primitive-strict.cirru` 会验证 strict 模式拒绝项目源码直接调用 `&get-raw`，并报告稳定的 `E_RAW_PRIMITIVE_IN_TYPED_CODE`。
- `erased-generic-relation-strict.cirru` 会验证 strict 模式拒绝把 Dynamic 实参传入重复泛型关系，并报告稳定的 `E_ERASED_GENERIC_RELATION`。

## 自动化测试

这些 fixture 已接到 Rust 测试里，会随 `cargo test` 一起运行。

- schema mismatch fixtures：断言最终错误文本包含 `E_SCHEMA_DEF_MISMATCH`
- call-site arg mismatch fixtures：断言产生 `W_FN_ARG_TYPE_MISMATCH` / `W_GENERIC_WHERE_BOUND_MISMATCH`
- type-slot hard-fail fixtures：断言错误文本包含具体 slot 绑定失败原因
- strict type-slot fixture：断言错误文本包含 `E_UNBOUND_TYPE_SLOT`、slot 名称与 schema 路径
- strict whole-Dynamic fixture：断言错误文本包含 `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA`、schema 根路径与 `Fn` 迁移建议
- strict Dynamic nominal-method fixture：断言错误文本包含 `E_DYNAMIC_POSTFIX_METHOD`、method 名称、receiver schema 与低层 helper 迁移建议
- strict general Dynamic method fixture：断言错误文本包含 `E_DYNAMIC_POSTFIX_METHOD`、method 名称、receiver-loss 分类与 typed adapter 迁移建议
- strict raw primitive fixture：断言错误文本包含 `E_RAW_PRIMITIVE_IN_TYPED_CODE`、primitive 名称与 typed public API 迁移建议
- strict erased-generic fixture：断言错误文本包含 `E_ERASED_GENERIC_RELATION`、callee、参数位置、泛型变量与 narrow/adapter 迁移建议

相关测试位于 [src/bin/calcit.rs](src/bin/calcit.rs)。

日常单独跑这组测试时，可以直接使用：

- `yarn test-fail`

这个命令会执行 `cargo test -q --bin calcit type_fail_`，专门覆盖这批 type-fail / schema-fail fixture 对应的测试。

## 当前相关 code

- `E_SCHEMA_DEF_MISMATCH`：定义与 `:schema` 的 `:kind` / `:args` / `:rest` 不匹配
- `E_UNBOUND_TYPE_SLOT`：strict 模式下可达定义的 schema 引用了未配置或未局部绑定的 type slot
- `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA`：strict 模式下可达 function 或直接进入预处理的 programmatic macro 既没有结构化根 schema，也没有嵌入式结构化契约；普通 legacy Snapshot macro 会更早被 loader 拒绝
- `E_DYNAMIC_METHOD_DISPATCH` / `E_DYNAMIC_POSTFIX_METHOD`：strict 模式下 method receiver 缺少可静态 specialization 的 schema、generic/slot 绑定或 typed FFI evidence
- `E_RAW_PRIMITIVE_IN_TYPED_CODE`：strict 项目源码手写 raw constructor/lookup/index primitive，且没有 compiler/macro origin 或匹配的 nominal layout evidence
- `E_ERASED_GENERIC_RELATION`：strict 模式下 Dynamic 实参擦除了 callee 声明的重复泛型关系
- `W_FN_ARG_TYPE_MISMATCH`：用户函数调用参数类型不匹配
- `W_METHOD_ARG_TYPE_MISMATCH`：静态方法调用参数类型不匹配
- `W_DYNAMIC_NOMINAL_METHOD_RECEIVER`：Option/Result method 的接收者仍为 Dynamic，需先收窄或在明确边界使用函数形式
- `W_PROC_ARG_TYPE_MISMATCH`：内建 proc 参数类型不匹配
- `W_CORE_FN_ARG_TYPE_MISMATCH`：`calcit.core` 函数参数类型不匹配
- `W_FN_RETURN_TYPE_MISMATCH`：函数声明返回类型与函数体实际返回类型不匹配
- `W_GENERIC_WHERE_BOUND_MISMATCH`：泛型绑定后的实际类型不满足 `:where` trait 约束
- type-slot fixture 额外覆盖：struct 绑定、未知 slot 绑定、重复绑定、strict 未绑定拒绝、跨程序加载的 slot 状态清理
