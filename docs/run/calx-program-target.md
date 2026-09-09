# Calx program backend

## English

### Direction

Calx is planned as an execution backend for the statically analyzable Calcit language core. A compilation starts from a configured entry set and includes its statically reachable definitions. It is no longer defined only as manually selected numeric kernels, although the existing strict kernel path remains the executable compatibility foundation.

The compiler consumes a typed, macro-expanded `CompiledProgram` snapshot. Calcit owns parsing, source/Snapshot handling, preprocessing, type analysis, macro expansion, reachability, lowering, lifecycle, and user-facing diagnostics. Calx owns strict program validation, core values and operations, typed imports, execution, traps, and source mapping.

Platform and application FFI stays outside Calx. I/O, JS/native modules, watch mode, long-lived connections, package loading, and capability state enter a compiled program only through explicit typed host imports.

### Coverage contract

`src/codegen/calx/coverage.rs` is the checked planning source of truth. Each type annotation, syntax form, and built-in operation has one owner:

- `compiler-only`: expanded, resolved, or erased before lowering;
- `calx-core`: implemented by typed lowering and the strict VM;
- `typed-host-import`: supplied externally with an exact signature;
- `planned`: valid static semantics assigned to a staged implementation slice;
- `excluded`: rejected with a structured source diagnostic.

Type and syntax variant classification uses exhaustive Rust matches. The larger `CalcitProc` inventory has an edition count test, so adding a proc requires reviewing and updating Calx coverage. These surface classifications are planning metadata, not eligibility proofs: concrete nested types must still reject Dynamic, Nil, unresolved generics, and unsupported fields. A later change must not weaken this check to a silent default.

The initial stages are:

1. the existing Number/Bool/`F64Buffer` kernel foundation;
2. program entries, `Unit`, string/tag values, and typed imports;
3. nominal struct/enum values, exhaustive match, and nominal `Option`/`Result`;
4. typed persistent List/Map/Set;
5. statically typed functions, closures, HOF, and arity features;
6. explicitly selected reference, error, and other stateful semantics.

The inventory is authoritative. A stage may be narrowed or reordered before implementation when validation or conformance evidence requires it.

### Hard boundaries

- Unsupported reachable code fails compilation; one compiled program does not silently mix Calx and Calcit execution.
- A Calx runtime trap is not retried in Calcit.
- Generic `Nil` is not used for completion, uninitialized storage, absence, or errors. Use `Unit` and nominal typed data.
- `Dynamic`, runtime `eval`, raw platform code, untyped FFI, `JsObject`/`AnyRef`, and unresolved dynamic dispatch are excluded from the first program contract.
- Calx does not duplicate Calcit's macro evaluator, type checker, package loader, scheduler, watch loop, or platform runtime.
- VM additions should be semantic primitives, not one opcode per Calcit built-in. Library behavior should remain ordinary compiled Calcit code where practical.

The current executable API and exact kernel ABI are documented in [Experimental Calx target](./calx-target.md). The broader program contract will receive a separate edition; it will not reinterpret `calcit-calx-kernel/1` or `/2`.

## 中文

### 方向

Calx 规划为可静态分析的 Calcit 语言核心的执行 backend。一次编译从配置入口集合开始，包含这些入口静态可达的 definitions。Calx 不再只定义为手工挑选的数值 kernel，但现有 strict kernel path 继续作为可执行的兼容基础。

编译器消费已经完成类型分析和宏展开的 `CompiledProgram` snapshot。Calcit 拥有 parsing、source/Snapshot、preprocessing、类型分析、宏展开、reachability、lowering、生命周期和用户侧诊断。Calx 拥有 strict program validation、核心值与操作、typed imports、执行、traps 和 source mapping。

平台与应用 FFI 留在 Calx 外部。I/O、JS/native modules、watch mode、长期连接、包加载和 capability state 只能通过显式 typed host imports 进入已编译程序。

### Coverage contract

`src/codegen/calx/coverage.rs` 是可检查的规划 source of truth。每个 type annotation、syntax form 和 built-in operation 只能有一个 owner：

- `compiler-only`：lowering 前已经展开、解析或消除；
- `calx-core`：由 typed lowering 和 strict VM 实现；
- `typed-host-import`：由外部以精确签名提供；
- `planned`：属于静态语义，并分配到一个实现阶段；
- `excluded`：带结构化源码诊断地拒绝。

类型与语法 variant 分类使用 Rust 穷尽匹配。较大的 `CalcitProc` 清单有 edition count 测试，所以新增 proc 必须同时审查并更新 Calx coverage。这些 surface classifications 是规划元数据，不是 eligibility proof；具体嵌套类型仍必须拒绝 Dynamic、Nil、未解析 generics 和不支持的字段。后续不得把该检查弱化为静默默认值。

初始阶段为：

1. 已有 Number/Bool/`F64Buffer` kernel foundation；
2. program entries、`Unit`、string/tag values 和 typed imports；
3. nominal struct/enum values、exhaustive match 和 nominal `Option`/`Result`；
4. typed persistent List/Map/Set；
5. 静态 typed functions、closures、HOF 和 arity features；
6. 显式选定的 reference、error 和其他 stateful semantics。

清单是权威依据。若 validation 或 conformance 证据要求，具体阶段可以在实现前收窄或调整顺序。

### 硬边界

- 可达代码中出现不支持能力时编译失败；同一个已编译 program 不静默混跑 Calx 与 Calcit。
- Calx runtime trap 后不在 Calcit 重跑。
- 通用 `Nil` 不表示完成、未初始化存储、缺失或错误；使用 `Unit` 和 nominal typed data。
- 首个 program contract 排除 `Dynamic`、运行期 `eval`、raw platform code、untyped FFI、`JsObject`/`AnyRef` 和无法静态确定的动态分派。
- Calx 不复制 Calcit macro evaluator、type checker、package loader、scheduler、watch loop 或 platform runtime。
- VM 应增加语义 primitives，而不是为每个 Calcit built-in 增加 opcode；适合的库行为继续由普通已编译 Calcit 代码表达。

当前可执行 API 与准确 kernel ABI 见[实验性 Calx target](./calx-target.md)。更广的 program contract 将使用独立 edition，不重新解释 `calcit-calx-kernel/1` 或 `/2`。
