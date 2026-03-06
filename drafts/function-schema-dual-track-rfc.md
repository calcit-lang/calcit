# RFC: 函数定义字段拆分与类型 DSL 数据化（纯数据迁移）

## 背景

当前 `CodeEntry` 以 `:code` 承载执行体，类型信息主要来自函数体内提示（例如 `hint-fn`、`assert-type`）。

这会带来几个问题：

- 类型分析依赖 body 逆向提取，受宏展开/重排影响；
- query/edit/tree 缺乏统一签名数据入口；
- 文档与覆盖率统计重复解析 `:code`，耦合较高。

## 目标

- 保留 `:code` 作为执行真源；
- 新增与 `:code` 平级的 `:schema` 字段；
- `:schema` 只使用 Calcit 现有数据结构（map/list/tag/symbol），不引入新语法；
- 类型检查和工具链逐步转为 `schema` 优先。

## 非目标

- 不改 `defn/defmacro` 执行语义；
- 不要求一次性补全历史 schema；
- 逐步迁移 `hint-fn/assert-type`。

## 数据模型提案

`CodeEntry` 中新增可选 `:schema`：

```cirru
%{} :CodeEntry
  :doc |...
  :examples $ []
  :code $ quote ...
  :schema $ quote $ {}
    :kind :fn
    :generics $ [] 'T 'U
    :args $ [] 'T :number
    :rest :number
    :return $ :: :tuple :ok 'U
    :where $ []
      :: 'Eq 'T
```

说明：

- `:schema` 可选，缺失时回退 body 抽取；
- `:schema` 与 `:code` 并存；
- schema 中不再要求 `:name`，函数名需要时可从 `:code` 提取；
- 字段值全部为现有 Calcit 数据结构；
- 支持以 `:optional` 开头包裹 schema，用于渐进迁移；
- `:rest` 支持元素类型短写（如 `:rest :number`），工具链按 `:: :list :number` 解释；
- schema 中未设置的字段按 `:dynamic` 语义处理；
- 可扩展 `:kind :macro/:proc`。

## 类型 DSL（纯数据表达）

类型值沿用现有表达：`:number`、`:: :list 'T`、`:: :fn ([] 'A 'B) (:: 'A) 'B`。

约定：`[]` 用于同构集合（如 generics 列表、args 列表）；`::` 用于固定结构且元素类型可不同的元组（如参数条目、约束条目、函数参数类型组）。

### 语法校验要求（强制）

- 所有 schema 示例必须通过 `cr` 解析校验；
- 命令使用 `cr demos/compact.cirru cirru parse-edn "..."`；
- PR 前至少校验改动过的 schema 示例。

说明：切换到 Cirru EDN 后，schema map 统一使用 `{}` 的 pair 写法。

主示例 parse 校验命令：

```bash
cr demos/compact.cirru cirru parse-edn "{} (:kind :fn) (:generics ([] 'T 'U)) (:args ([] 'T :number)) (:rest :number) (:return (:: :tuple :ok 'U)) (:where ([] (:: 'Eq 'T)))"
```

可选包裹示例（迁移期推荐）：

```bash
cr demos/compact.cirru cirru parse-edn "{} (:optional ({} (:kind :fn) (:args ([] :dynamic)) (:return :dynamic)))"
```

### 运行时数据验证（`cr eval` + `println`）

```bash
cr demos/compact.cirru eval "let ((schema ({} (:kind :fn) (:generics ([] 'T 'U)) (:args ([] 'T :number)) (:rest :number) (:return (:: :tuple :ok 'U)) (:where ([] (:: 'Eq 'T)))))) (println schema) (println (type-of schema)) , schema"
```

预期：`println (type-of schema)` 输出 `:map`，证明 schema 在运行时是普通数据。

### 基础类型

- `:number` `:string` `:bool` `:dynamic` ...
- 泛型变量：`'T`（推荐）

### 复合类型

- 列表：`:: :list T`
- 集合：`:: :set T`
- 映射：`:: :map K V`
- 元组/变体：`:: :tuple :tag T1 T2`
- 函数：`:: :fn ([] generics...) (:: args...) return`

### 约束表示

约束统一放在 `:where`，每条约束是一个 tuple：`:: Trait TypeVar`。

单变量多个约束：

```cirru
[]
  :: 'Eq 'T
  :: 'Show 'T
```

多个变量各自约束：

```cirru
[]
  :: 'Eq 'T
  :: 'Ord 'T
  :: 'Show 'U
```

说明：

- 同一个变量出现多条约束时表示“且”关系（必须同时满足）；
- 约束顺序不影响语义，但建议按变量分组，便于 review；
- 迁移期如暂不表达约束，可用 `:where $ []`；
- 若未来引入复杂约束组合，建议保持 tuple 结构并新增显式 tag，而不是复用字符串。

```cirru
:: Eq 'T
:: Show 'T
```

### 完整示例

```cirru
{}
  :kind :fn
  :generics $ [] 'A 'B 'C
  :args $ []
    :: :fn
      [] 'A 'B 'C
      :: 'A 'B
      'C
    :: :list 'A
    :: :list 'B
  :return $ :: :list 'C
  :where $ [] (:: 'Eq 'A)
```

## 一致性规则

- schema 与 `hint-fn/assert-type` 同时存在时，schema 优先；
- 对两者做一致性检查，不一致先 warning（后续可升 error）；
- 只有 body 提示时保持现有行为；
- 两者都没有时按动态类型策略。

## 实施分期

### Phase 1（MVP）

1. 扩展 `CodeEntry` 可选 `:schema`；
2. `snapshot` 兼容读写（无 schema 不报错）；
3. 新增 `parse_schema_data`，校验字段结构合法性；
4. query/type-coverage 优先读取 schema。

### Phase 2

1. `preprocess` 增加 schema/body 一致性检查；
2. `docs check-md` 增加“缺 schema/冲突”提示；
3. 优先补 core 高频函数 schema。

### Phase 3

1. 类型推断默认 schema 优先；
2. `hint-fn/assert-type` 逐步转为补充约束；
3. 评估新定义模板默认生成 schema。

## 与当前片段的关系（`hint-fn/assert-type`）

现有代码：

```cirru
defn %err (message)
  hint-fn $ return-type :tuple
  assert-type message :dynamic
  %:: Result :err message
```

对应 schema（持久化时使用 quote 包裹）：

```cirru
quote $ {}
  :kind :fn
  :generics $ []
  :args $ [] :dynamic
  :return :tuple
  :where $ []
```

## 验收标准（MVP）

- `snapshot` 兼容旧数据；
- 新增 schema 不影响运行与宏展开；
- `query`/类型覆盖可读 schema 并回退；
- RFC 示例可被 `cr ... cirru parse-edn` 成功解析；
- `cargo test` 与 `yarn check-all` 通过。
