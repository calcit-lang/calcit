# RFC: `todo!` 未实现占位表达式与静态提醒

状态：Draft  
日期：2026-08-14  
关联：`08-14-architecture-scaffold-rfc.md`、`03-05-function-schema-dual-track-rfc.md`、`07-26-static-semantic-analysis-rfc.md`

## 1. 概要

新增与 Rust `todo!()` 对应的 Calcit 占位表达式：

```cirru
defn validate-order (order)
  todo! "|implement order validation"
```

`todo!` 表示“此路径刻意尚未实现”，同时满足三类契约：

- 类型检查：可出现在任意期望返回类型的位置，不产生返回类型 mismatch；
- 静态诊断：产生带精确 definition/path 的 `W_TODO`，提醒 Agent 仍有未完成工作；
- 运行时：一旦执行到该路径，native/JS 明确抛出 TODO error，WASM 执行 `unreachable` trap。

它不是 `raise` 的别名。`raise` 表示程序设计中的普通异常路径，静态分析不应据此推断代码未完成；`todo!` 表示开发状态，必须能被结构化查询和完成门禁识别。

## 2. 表面语法

第一版支持零或一个 String 参数：

```cirru.no-check
todo!
todo! "|implement app.order/validate-order"
```

约束：

- 名称使用 `todo!`；`!` 是 Calcit 命名约定的一部分，不采用 Rust 括号宏语法；
- message 省略时使用稳定默认文本 `TODO: implementation is pending`；
- message 必须是静态 String literal，保证 analyzer 无需执行代码即可输出原因；
- 多参数、Tag、动态拼接或 metadata map 第一版拒绝，避免占位表达式演变成日志 API；
- scaffold 需要携带 planned calls 时，将它们格式化进静态 message，同时仍以 architecture graph 作为机器可读来源。

## 3. 类型语义

### 3.1 Never/bottom

目标语义中，`todo!` 的表达式类型是内部 `Never`（bottom type），不是 `Unit` 或任意伪造的业务类型：

```cirru
defn load-user (id)
  ; schema return: User
  todo! "|load user"
```

规则：

- actual `Never` 满足任意 expected type，因此不产生 `W_FN_RETURN_TYPE_MISMATCH`；
- `if` / `match` 的一个分支为 `Never`、另一分支为 `T` 时，整体推断为 `T`；
- `Never` 不参与泛型变量绑定，不把 `T` 推断成 `Never`；
- `Never` 没有运行时 value，也不能构造或存入 collection；
- 第一阶段只作为 compiler internal annotation；不承诺用户可在 schema 中显式写 `'Never`。

当前基础实现以 compiler-known diverging proc 的 `Dynamic` 返回签名避免伪造的返回类型 mismatch，同时由 `W_TODO` 保留“未完成”信息；它尚未改变 `if`/泛型的类型 join。完整 `Never` 是下一阶段的类型系统工作，届时替换该内部表示，而不改变表面语法或诊断协议。

### 3.2 与控制流的关系

`todo!` 是 diverging expression。preprocessor 在它之后仍可保留源码用于展示，但控制流和类型 join 不要求该表达式返回。第一版不借此实现完整的 unreachable-code warning；这可以在未来与 `raise`、`quit!` 的控制流分析统一处理。

## 4. 静态诊断

每个 `todo!` 产生：

```text
[W_TODO] TODO remains in `app.order/validate-order`: implement order validation
  at app.order/validate-order @3
```

Cirru EDN diagnostic 至少包含：

```cirru
{}
  :code :W_TODO
  :severity :warning
  :namespace 'app.order
  :definition 'validate-order
  :path $ [] 3
  :message "|implement order validation"
```

诊断策略：

- `W_TODO` 是 completion warning，不是 type warning；同一位置不得再产生伪造的返回类型 mismatch；
- scaffold dry-run/apply 把它列入 expected warnings，不把新 stub 当成 apply conflict；
- `cr --check-only` 遵循当前严格 warning 策略：可创建 scaffold，但仍有 reachable TODO 时检查返回非零，Agent 不能宣称功能完成；
- `cr analyze check-types` 扫描所选 Snapshot definition，将 TODO 数量和 diagnostics 纳入 human/Cirru EDN 报告，即使节点暂时不从 entry 可达；
- 后续若引入 warning severity/allow-list，`W_TODO` 的默认 completion gate 仍应为 deny，显式探索性运行才允许降级；
- Cirru EDN stdout 保持单个 value；JSON 只作为现有工具兼容投影，human warning 与普通命令提示走 stderr。

参数契约也由 compiler-known proc 统一执行：`todo!` 只接受零个参数，或一个
静态 String literal；非 literal 参数和多余参数在 preprocessing 阶段分别报告
Type/Arity 错误并阻止 codegen。native、JavaScript、WASM backend 仍保留同样的
校验作为防御性边界，不把非法调用静默降级为 unconditional trap。

## 5. 运行时与 codegen

即使静态门禁通常会先发现 TODO，各后端仍必须定义一致的防御行为：

| 后端 | 行为 |
|------|------|
| native | 返回 `CalcitErrKind::Effect`，message 以 `TODO:` 为前缀并附 call stack |
| JavaScript | 生成 `throw new Error("TODO: ...")` |
| WASM | 在可选 host log 后执行 `unreachable`；第一版至少保证 trap，不返回伪值 |
| IR | 保留明确的 todo/diverge 节点，不能降为 `nil` |

`try` 是否能捕获 native/JS TODO 第一版沿用普通 runtime error 机制；静态 `W_TODO` 不因外层存在 `try` 而消失，因为捕获异常不代表实现已经完成。

## 6. 实现形态

推荐将 `todo!` 作为 compiler-known builtin/syntax，而不是 calcit-core 普通函数：

- parser/name resolution 能稳定识别，不受局部同名 binding 影响；
- type inference 可直接返回 internal `Never`；
- preprocessor 能在表达式位置产生精确 `W_TODO`；
- JS/WASM codegen 可直接生成 diverging operation；IR 的显式 todo/diverge 节点留在后续实现；
- runtime 不需要用普通 `raise` 猜测某段字符串是否以 TODO 开头。

当前实现采用 `CalcitProc::Todo`：preprocessor 对它生成精确 `W_TODO`，Dynamic signature 仅作为完整 `Never` 前的临时内部兼容表示，不能把 warning 当作可忽略的普通动态边界。

## 7. 与 scaffold 的关系

architecture scaffold 默认生成：

```cirru
defn validate-order (order)
  todo! "|implement app.order/validate-order; planned calls: app.order/order-total"
```

- doc/schema/params 正常写入 `CodeEntry`；
- `:scaffold` tag 与 `W_TODO` 分工：tag 表示 definition 来源，warning 表示代码中仍存在未实现路径；
- 完成实现时必须移除对应 `todo!`；是否同时自动移除 `:scaffold` tag 由 scaffold completion check 决定；
- planned edges 仍来自 architecture graph，不把 message 当成可解析协议；
- 实际 call graph 在实现前可以没有 planned calls，drift report 应把这种缺失标记为 `pending`，而不是伪造不可执行调用。

## 8. 分阶段实现

### Phase A：基础可用版本（已实现）

- 注册 `todo!`；
- native runtime TODO effect；
- 精确 `W_TODO` preprocessing diagnostic；
- function schema return context 测试；
- 完整 internal Never/bottom、branch/generic 规则留给后续控制流分析。

### Phase B：分析与后端

- `analyze check-types` 全 Snapshot TODO 扫描及 Cirru EDN machine result；
- JS/WASM codegen 已实现；IR 的显式 todo/diverge 节点仍待实现；
- `--check-only`、eval、test 和 codegen warning 行为测试；
- Agent 文档与 error-handling 文档。

### Phase C：scaffold 接入

- scaffold 默认 `--stub todo`；
- generated/expected warning 分类；
- TODO 清零和 `:scaffold` tag 完成检查；
- architecture drift 的 pending edge 表达。

## 9. 验收

- `todo!` 可作为 `'Number`、`'String`、Struct、Enum、generic 和 `Unit` 函数的返回表达式，不产生类型 mismatch；
- `if condition (todo! "|left") 1` 推断为 Number；
- 每处 TODO 都产生一个带稳定 `W_TODO`、FQN、path 和 message 的 diagnostic；
- `analyze check-types` 能发现当前 entry 不可达 definition 内的 TODO；
- scaffold apply 可成功创建 TODO stub，但完成门禁不会在 `W_TODO` 尚存时通过；
- native、JS、WASM 执行到 TODO 时均中止，不返回 `nil` 或默认值；
- `raise "|TODO..."` 不产生 `W_TODO`，普通异常语义保持不变；
- shadowing 或同名用户 definition 不会被误判为 compiler TODO；
- Cirru EDN stdout 保持单个 value，兼容 JSON renderer 不改变该纯净度契约；
- 全部核心、CLI、JS 和 WASM 回归通过。
