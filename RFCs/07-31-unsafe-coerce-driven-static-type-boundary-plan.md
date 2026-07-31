# RFC: `unsafe-coerce` 驱动的动态边界治理与静态类型强化计划

状态：Draft
日期：2026-07-31
关联：`07-26-static-semantic-analysis-rfc.md`、`07-08-ffi-features-and-js-object-type-rfc.md`、`06-01-generic-binding-unification-rfc.md`

## 1. 现状与目标

我们将本阶段主目标聚焦在类型边界治理，而不是单纯推进 WASM 后端。
针对 FFI（特别是 JS object）和外部大树形结构中大量动态值的问题，`unsafe-coerce` 要成为“静态分析可利用的类型入口点”，降低 `:dynamic` 退化。

目标如下：

- 让动态边界明确化：能指出哪些值是故意进入静态盲区，哪些是可治理缺口；
- 提升静态可用信息：`unsafe-coerce` 后的类型关系要能在后续分析中被持续传播；
- 降低告警噪音：减少因隐式动态而产生的误报，转而给出可执行的收紧建议。

## 2. 问题定义

当前主要痛点是三类：

1. 外部输入（JS Object、外部树形数据）一旦进入，会被频繁打散成 `:dynamic`；
2. `unsafe-coerce` 当前只附加声明类型，不转换值，也不提供运行时校验，静态分析无法区分“推导所得”与“用户信任声明”；
3. 未通过显式边界收敛的动态传播，会导致高阶函数、容器、回调、trait 调用处丧失可验证关系。

## 3. 设计思路

我们将类型边界分为五档，贯穿检查与诊断链路：

- `sealed-boundary`：明确的外部边界，需经过显式操作收窄；
- `trusted`：`unsafe-coerce` 给出的用户信任声明，可传播但不视为运行时证明；
- `validated`：由未来的实际校验操作建立的证据；
- `unvalidated`：未经过转换但可继续跟踪的弱上下文；
- `dynamic`：有意接受的动态盲区（可选 opt-in）。

`unsafe-coerce` 必须具备两个产出：

- 运行时行为：保持原值不变，不承诺验证；
- 静态元信息：来源类型、声明目标类型、边界位置和 `trusted` 置信等级。

实际完成运行时验证的能力应使用独立操作，并把置信等级提升为 `validated`，不能由 `unsafe-coerce` 冒充。

## 4. 规范草案

### 4.1 `unsafe-coerce` 的静态签名

`unsafe-coerce` 调用应显式携带目标类型表达式，并在静态树或旁路分析元数据中记录：

- `from-type`：调用点推断来源；
- `to-type`：目标类型表达式；
- `confidence`：固定为 `trusted`，不可标为 `validated`；
- `evidence`：字段集合、泛型约束、变体等。

示例：

```cirru
unsafe-coerce user-json :js-object
unsafe-coerce user-json (:: :record User)
unsafe-coerce tree-data (:: :list (:: :ref TreeNode))
```

### 4.2 结构化构造的类型加强策略（enum / struct）

当调用头能够静态确认就是某个 `defstruct` 或 `defenum` 定义时，类型收敛可走“无缝构造”策略，减少 `::` / `%::` / `%{}` 的显式写法。普通 record 实例或 enum tuple 即使携带相同类型信息，也不得被当作构造器：

- `enum`：在可推断为 enum 的位置，允许直接写 `Result :ok value`，由类型驱动 rewrite 成 `%:: Result :ok value`；
- `struct`：在可推断为结构体的上下文，允许按字段对写 `Person :name |Alice :age 20`，并在类型上下文中改写为 `%{} Person :name |Alice :age 20` 的有序结构构造；
- 规则约束仍保留：
  - `struct` 必须是偶数个参数，按 key/value 成对出现；
  - 字段不能重复；非 `:optional` 字段不能缺失；
  - `enum` 的 tag 与 payload 必须匹配该 enum 的 variant；
  - 任何 key 不在目标 struct 字段中的，回退到原有调用并给出 warning。

示例：

```cirru
let
    maybe-ok $ Result :ok 1
    person $ Person :name |Alice :age 20
  ...
```

负向示例（应保持原样并给 warning）：

```cirru
Person :name |Alice :age
Result
Result :bad 1
Person :email |x
Result ok 1
kitty .rename |LagopusB
```

说明：

- `Person :email |x`：字段不在结构定义中 → 回退并 warning；
- `Person :name |Alice :age`：奇数参数 → 回退并 warning；
- `Result ok 1`：enum 首参不是 tag（需 `:ok`）→ 回退并 warning；
- `kitty .rename |LagopusB`：记录/结构方法调用不能被误判为构造调用。

### 4.3 动态边界的使用规则

1. 来自 FFI/主机接口的值默认进入 `sealed-boundary`；
2. 若值要进入“纯 Calcit 计算路径”，必须通过 `unsafe-coerce` 进入可信声明类型，或通过实际校验操作进入已验证类型；
3. 对容器与回调关系，`unsafe-coerce` 后应尽量保留内含关系（如 `:: :list T`、`:: :fn` 的参数/返回关联）；
4. 对于无法构建精确信息的对象（如完全开放结构），允许 `:dynamic`，但必须记录“故意动态”标签与边界位置。

### 4.4 与现有警告体系对齐

- 引入/保留告警码：
  - `W_SEALED_BOUNDARY_PASS`
  - `W_MISSING_COERCE`
  - `W_COERCE_COVERAGE_GAP`
  - `W_DYNAMIC_EXIT`
- `unsafe-coerce` 使用不足的动态传播点应由 `W_MISSING_COERCE` 定位，并给出可替换的目标类型示例；
- `unsafe-coerce` 后仍不能稳定推断的情况，转为 `W_COERCE_COVERAGE_GAP` 并输出缺失字段/参数位置。

### 4.5 与函数签名协同

`unsafe-coerce` 不是为了替代 schema，而是使 schema 可恢复：

- 大树节点进入列表/record 时，优先映射为命名结构体或 enum；
- 回调参数在边界后保持 `:fn` 的关系（arg/return / generics / where）；
- 若同一结构反复出现，鼓励提升为 `defstruct` + `:where` + 类型参数，减少重复 `:dynamic`。

## 5. 实施路线

### Phase 1（1~2 周）：把边界变可见

- 明确 `unsafe-coerce` 在静态元数据中的记录模型，并保证 local 场景不会擦除边界节点；
- 增加 `sealed-boundary` / `trusted` / `validated` / `unvalidated` / `dynamic` 标签在诊断中可见；
- 在 `type-at` 与 `check-types` 输出中显示 `unsafe-coerce` evidence 链路；
- 增加 `unsafe-coerce` 相关正向用例：JS Object → 记录/列表/enum。

### Phase 2（2~3 周）：增强 `unsafe-coerce` 的传播能力

- 在 container / fn / struct / enum 场景保留内含关系；
- 增强跨文件、跨 def 的类型传播，支持大树结构递归字段推断；
- 给 FFI 边界添加可执行的“首入场 unsafe-coerce 指南”（必须声明哪些层级，以及哪些位置需要真实校验）。

### Phase 3（1~2 周）：治理现有动态盲区

- 统计 `:dynamic` 产生原因：未转换、边界故意、兼容旧行为；
- 逐步把“可修复” `:dynamic` 转为结构化 `unsafe-coerce` 或真实校验，保留“故意动态”最小集合；
- 在 `analyze weak-types` 中增加类型边界报告：按定义、路径、优先级排序。

## 6. 验收指标（建议）

1. 关键 FFI 边界进入非纯路径时，`type-at` 能标出 `sealed-boundary`；
2. 对树形/树状 JSON 数据，核心字段至少一层以上可从 `:dynamic` 收窄；
3. 同一入口内的未转换动态访问点告警下降且可定位；
4. 新增样例在 `check-types` 下可通过，不产生“类型丢失无法解释”的模糊告警。

## 7. 风险与缓解

- 风险：`unsafe-coerce` 过度强约束导致现有生态迁移成本上升。
  缓解：保持 warning-first，逐步收紧。
- 风险：分析时间上升。
  缓解：对 `unsafe-coerce` evidence 做缓存并只在必要路径传播。
- 风险：FFI 数据本身不稳定结构导致验证失败。
  缓解：支持“部分声明 + `:dynamic` fallback”，让用户明确知道保留了什么边界。

## 8. 依赖关系

优先参考并复用：

- `06-01-generic-binding-unification-rfc.md`
- `07-26-static-semantic-analysis-rfc.md`
- `07-08-ffi-features-and-js-object-type-rfc.md`
- 现有类型诊断体系与 `analyze weak-types` 输出规范
