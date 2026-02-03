# Runtime Traits for Calcit (Draft)

> 目标：在运行时提供一套 Trait 机制，即使无法静态检查，也能把错误前移到调用点，并允许类型级别的能力声明（如 `Show`、`Add`、`Multiply` 等）。

## 范围评估

### 1) 核心数据结构与运行时

- **`Calcit` 新增变种**：`Trait`（以及可能的 `TraitImpl` 数据结构）。
- **运行时上下文**：不单独引入 registry，内置 Trait 直接放在 `calcit.core`，其他 Trait 作为普通值附着在各自命名空间上。
- **调用分派**：方法调用/操作符调用在当前命名空间与 `calcit.core` 中解析 Trait 值，再执行绑定实现。
- **Trait 实现承载**：Traits 以组合方式使用，携带实现时统一使用 `Vec`；即便内置类型，查询 trait 实现也返回数组，内部实现目前用 record 承载。
- **错误模型**：为 trait 查找失败、约束不满足等错误引入新的错误类别与消息格式。

### 2) 标准库/内建能力定义

- **内建类型能力**：
  - `Show`：所有类型默认实现（或至少 `nil/bool/number/string/list/map/set/tuple`）。
  - `Add`/`Multiply`：`number` 实现；`string` 可选实现 `Add`（拼接）但需明确语义。
  - 其他可选：`Compare`、`Eq`、`Hash`、`Len`、`Index` 等。
- **内建函数/语法桥接**：
  - `+`, `*`, `str` 等需要改为通过 trait 调用或保留内建分支 + trait 兜底。
  - 现有 `method`/`record`/`tuple` 行为需要确定与 trait 的交互规则。

### 3) 语言层定义与语法

- **Trait 定义**：动态类型前提下，可先按普通值定义 Trait，后续再考虑是否需要专用语法（如 `deftrait`）。
- **Trait 实现**：满足 Trait 的实现直接使用 record 表达，运行时通过 `with-class` 之类的机制挂上去（未来可能调整）。
- **Trait 约束表达**：考虑引入 `assert-trait`，用法类似 `assert-type`，用于声明约束与在调用处触发运行时检查。

### 4) 运行时行为变更

- **左移报错**：
  - 在调用处，若目标类型未实现 trait，直接抛错（避免进入执行体）。
  - 在加载时注册 trait 实现并检测冲突（重复实现、签名不匹配等）。

## 修改范围与复杂度

### 高风险/广泛影响

- `Calcit` enum 变更（新增变种） → **所有 pattern match 需要更新**。
- `runner`/`preprocess`/`builtins` 的调用逻辑需要接入 trait 解析。
- `codegen` 需要考虑 trait 分派（JS backend 与 runtime 协议）。
- 现有内建操作（`+`, `*`, `.` method）需重新定义分派规则。

### 中等影响

- `calcit.core` 标准库结构可能需要新增 trait 定义与实现。
- 错误与警告格式新增类型。

### 低风险

- 文档、示例与 tests 的补充。

## Breaking Changes 预估

- `Calcit` 匹配逻辑：新增变种会导致编译错误与运行时路径调整。
- 内建操作行为：如果 `+/*` 改为 trait 分派，某些动态调用会改变错误时机。
- `method` 分派：若 trait 与 class record 方法冲突，需要定义优先级（可能改变现有行为）。
- `str`/`format` 与 `Show` 的统一：输出可能略有不同。

## 设计决策待确认

1. Trait 分派优先级：
   - 内建实现 vs trait 实现 vs record method
2. Trait 的定义方式：
   - 新语法 `deftrait` / `defimpl` vs 复用 record/defn
3. Trait 是否支持泛型：
   - 初期可不支持，后续扩展。
4. 多实现冲突处理：
   - 同一类型是否允许多个实现？冲突如何处理？

## 建议实施阶段（草案）

### Phase 1：基础结构

- 引入 `Trait` 数据结构，内置 Trait 放在 `calcit.core`（不新增 registry）。
- 内建 `Show` + `Number` 的 `Add/Multiply` 实现。
- 调整 `+`/`*` 使用 trait 分派（保留内建快速路径）。

### Phase 2：语言层支持

- `deftrait` / `defimpl` 语法与 runtime 注册。
- `assert-trait` / `requires` 运行时检查。

### Phase 3：扩展与稳定

- 覆盖更多内建类型能力。
- 增加 tests（cirru 文件）覆盖 trait 注册、冲突、失败路径。

## 测试补充建议（cirru）

- `test-traits.cirru`：
  - `Show` for number/string/list/map
  - `Add`/`Multiply` for number
  - 未实现时的错误提示
  - 多实现冲突/重复注册

---

> 备注：该方案涉及 runtime/stdlib/codegen 全链路改造，建议先从最小可运行集开始迭代。
