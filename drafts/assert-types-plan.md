# Calcit 局部变量类型标记开发计划

> 本计划文档聚焦交付节奏、风险与任务拆解；功能背景与语法示例请参见 `drafts/generic-types.md`。

## 已完成

### 阶段性成果（截至 2026-01-08）

- ✅ **阶段 1：数据结构准备** — `CalcitLocal`/`CalcitFn` 结构已扩展，`cargo test` 全量通过。
- ✅ **阶段 2：语法解析** — `assert-type`、`hint-fn` 全量识别并带单元测试回归。
- ✅ **阶段 3：类型传播** — `ScopeTypes` 作用域链稳定，`calcit/test-types.cirru` 可复现类型标注。
- 🚧 **阶段 4：Record/Enum 静态校验** — Record 字段检查已上线，Enum/推断仍在推进。

### 现有实现能力摘要

1. 变量与函数元数据：`CalcitLocal.type_info`、`CalcitFn.return_type/arg_types` 会输出到 IR，供 CLI/工具链消费。
2. 语法落地：`assert-type` 在预处理阶段注入类型且运行时代价为 `Nil`；`hint-fn $ return-type ...` 由 `defn` 解析并写入 `CalcitFn.return_type`，同时兼容 `hint-fn async`。
3. Record 校验：`&record:get` 与 `user.-field` 调用若访问未声明字段会抛出 `LocatedWarning`，并附上可用字段列表。
4. 告警与演示：`calcit/test-types.cirru`、`js-out/program-ir.cirru` 中可直接观察 `:type-info`/`return-type` 字段。

### 测试与示例覆盖

- 单元测试：共 11 个，覆盖语法解析、作用域传播、Record 字段合法/非法访问等路径。
- Cirru 示例：`calcit/test-types.cirru` 展示多种类型标注及 return-type 提示。
- 典型 Record 告警样例：

```rust
let test_record = Calcit::Record(CalcitRecord {
  name: EdnTag::from("Person"),
  fields: Arc::new(vec![EdnTag::from("age"), EdnTag::from("name")]),
  values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
  class: None,
});
scope_types.insert(Arc::from("user"), Arc::new(test_record));
```

> 当代码调用 `(&record:get user :email)` 或 `user.-email` 时，即会产生 `Field 'email' does not exist in record 'Person'` 的预处理警告。

### 已完成任务（Task 4.1~4.5）

- 4.1：完成 `defrecord` 管线梳理与字段读取验证 demo。
- 4.2：在 `program` 层实现 `lookup_record_fields` 类接口，用于 `ScopeTypes` 填充。
- 4.3：`preprocess_list_call`/`Method(Access)` 均可触发字段验证并输出 `LocatedWarning`。
- 4.4：补齐 `validates_*field_access` 系列测试与 Cirru 案例。
- 4.5：`cr --check-only` 能在 demo 中捕获故意写错的字段，验证静态告警链路。

## 未完成

### 风险与考量

- **宏展开顺序**：需确保在宏完全展开后仍能关联类型信息，避免遗漏 `assert-type`/`hint-fn`。
- **性能开销**：预处理阶段的哈希查找可能导致编译变慢，需要关注大型项目的影响。
- **复杂类型**：当前仅覆盖基础类型与 Record；泛型/联合类型暂未纳入，需规划扩展路线。

### 阶段性开发计划（展望）

| 阶段                     | 目标                                                                          | 交付物                                    |
| :----------------------- | :---------------------------------------------------------------------------- | :---------------------------------------- |
| 阶段 1：数据结构准备     | 扩展 `CalcitLocal` 与相关派生，实现 `type_info` 字段并占位 `assert-type` 语法 | 可编译版本 + 基础单测                     |
| 阶段 2：语法解析         | Cirru→Calcit 过程识别 `assert-type`、`hint-fn` 并保留 AST 注解                | `cr --check-only` 示例                    |
| 阶段 3：类型传播         | `preprocess_*` 传递 `ScopeTypes` 并在 `Local` 中带上 `type_info`              | demo 函数 + `&inspect-type` helper        |
| 阶段 4：Record 静态校验  | 校验 `Record` 字段、提供错误定位、覆盖 `&record:get`/`.-field`                | 合法/非法调用示例 + `cr query error` 输出 |
| 阶段 5：运行时与内置函数 | `&inspect-type` 原语、内置函数签名、`unknown` 回退策略                        | 运行时示例 + 文档/测试                    |

> 阶段 1~3 已交付；表格保留原路线，方便追踪阶段 4/5 的补完情况。

### 阶段 4 剩余工作

1. **Record 字面量推断**：识别 `&new-record`/`&%{}` 构造，自动写入 `ScopeTypes`。
2. **Enum 变体验证**：校验 `tag-match`/`defenum` 变体名称与参数个数。
3. **`hint-fn` 元信息消费**：
   - 批量补齐 `return-type` 注解（核心库以外）。
   - 在 `program-ir`/`cr query` 中展示返回值期望，为调用点校验铺路。

### 待办任务清单（按主题拆分）

#### Enum variant 支持（可选）

- [ ] **任务 4.6**：调研/确认 `defenum` 现状，确定是实现新语法还是复用 Record。
- [ ] **任务 4.7**：为 `tag-match`/`case` 实现变体验证并输出 `LocatedWarning`。

#### 内置函数类型提示

- [ ] **任务 4.8**：设计 `CalcitProc` 签名描述结构（字符串或 Calcit 值）。
- [ ] **任务 4.9**：为常用 proc（`+`、`map`、`assoc` 等）补充签名并加单测。
- [ ] **任务 4.10**：预处理阶段读取签名，对参数数量/类型做最小校验。

#### 运行时工具与诊断

- [ ] **任务 4.11**：实现 `&inspect-type` 原语，便于 REPL 观察类型标注。
- [ ] **任务 4.12**：在 `LocatedWarning::print_list` 等路径输出变量 `type_info`，提升可调试性。

#### 文档与示例

- [ ] **任务 4.13**：撰写 `docs/type-annotations.md`，汇总语法与 Record 校验示例。
- [ ] **任务 4.14**：更新 `README`/changelog，记录类型标记能力上线情况。

#### 集成测试与性能

- [ ] **任务 4.15**：运行全量测试，观察大项目中的预处理性能。
- [ ] **任务 4.16**：必要时对 `ScopeTypes` 克隆等热路径做 profiling/优化（例如引入 `Arc` 共享）。

### 执行顺序建议

1. **阶段 4 主线**：Record 字面量推断 → Enum 变体验证 → `hint-fn` 元信息消费。
2. **配套能力**：运行时工具（4.11/4.12）与内置函数签名（4.8~4.10）可交叉进行。
3. **收尾**：文档/示例（4.13/4.14），随后执行性能与集成测试（4.15/4.16）。

_评估日期：2026-01-08 · 当前状态：阶段 3 已完成，阶段 4 进行中_
