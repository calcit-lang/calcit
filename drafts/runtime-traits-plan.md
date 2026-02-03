# Runtime Traits for Calcit (Draft)

> 目标：在运行时提供一套 Trait 机制，即使无法静态检查，也能把错误前移到调用点，并允许类型级别的能力声明（如 `Show`、`Add`、`Multiply` 等）。

## 内置 Trait 设计

参考 Haskell (Eq, Ord, Show, Num, Foldable, Functor) 与 Rust (Display, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Iterator, Add, Mul, From/Into) 的设计，结合 Calcit 的动态类型特性，定义以下内置 Trait：

### 核心 Trait 分类

#### 1. 显示与调试 (Display & Debug)

| Trait     | 方法签名                 | 描述                 | Haskell 对应 | Rust 对应 |
| --------- | ------------------------ | -------------------- | ------------ | --------- |
| `Show`    | `(show x) -> :string`    | 人类可读的字符串表示 | `Show`       | `Display` |
| `Inspect` | `(inspect x) -> :string` | 调试用的详细表示     | `Show`       | `Debug`   |

**默认实现**: 所有内置类型 (`nil`, `bool`, `number`, `string`, `tag`, `symbol`, `list`, `map`, `set`, `tuple`, `record`, `fn`) 都有默认实现。

#### 2. 相等与比较 (Equality & Ordering)

| Trait     | 方法签名                | 描述                   | Haskell 对应 | Rust 对应   |
| --------- | ----------------------- | ---------------------- | ------------ | ----------- |
| `Eq`      | `(eq? x y) -> :bool`    | 相等性判断             | `Eq`         | `PartialEq` |
| `Compare` | `(compare x y) -> :tag` | 返回 `:lt`/`:eq`/`:gt` | `Ord`        | `Ord`       |

**默认实现**:

- `Eq`: 所有值类型
- `Compare`: `number`, `string`, `tag`, `list`（字典序）

#### 3. 算术运算 (Arithmetic)

| Trait      | 方法签名               | 描述      | Haskell 对应 | Rust 对应 |
| ---------- | ---------------------- | --------- | ------------ | --------- |
| `Add`      | `(add x y) -> 'T`      | 加法/拼接 | `Num (+)`    | `Add`     |
| `Subtract` | `(subtract x y) -> 'T` | 减法      | `Num (-)`    | `Sub`     |
| `Multiply` | `(multiply x y) -> 'T` | 乘法      | `Num (*)`    | `Mul`     |
| `Divide`   | `(divide x y) -> 'T`   | 除法      | `Fractional` | `Div`     |
| `Negate`   | `(negate x) -> 'T`     | 取负      | `Num negate` | `Neg`     |

**默认实现**:

- `number`: 全部算术 Trait
- `string`: `Add`（字符串拼接）
- `list`: `Add`（列表连接）

#### 4. 集合操作 (Collection)

| Trait      | 方法签名                      | 描述          | Haskell 对应 | Rust 对应    |
| ---------- | ----------------------------- | ------------- | ------------ | ------------ |
| `Len`      | `(len x) -> :number`          | 长度/大小     | `length`     | `len()`      |
| `Empty`    | `(empty? x) -> :bool`         | 是否为空      | `null`       | `is_empty()` |
| `Contains` | `(contains? x item) -> :bool` | 包含检查      | `elem`       | `contains()` |
| `Get`      | `(get x key) -> 'V`           | 按键/索引取值 | `lookup`     | `get()`      |

**默认实现**: `string`, `list`, `map`, `set`

#### 5. 可迭代 (Iterable)

| Trait        | 方法签名                   | 描述      | Haskell 对应 | Rust 对应  |
| ------------ | -------------------------- | --------- | ------------ | ---------- |
| `Foldable`   | `(fold x init f) -> 'A`    | 折叠/归约 | `Foldable`   | `fold()`   |
| `Mappable`   | `(fmap x f) -> 'T`         | 映射转换  | `Functor`    | `map()`    |
| `Filterable` | `(filter-by x pred) -> 'T` | 过滤      | `filter`     | `filter()` |

**默认实现**: `list`, `map`, `set`

#### 6. 哈希与克隆 (Hash & Clone)

| Trait   | 方法签名              | 描述   | Haskell 对应 | Rust 对应 |
| ------- | --------------------- | ------ | ------------ | --------- |
| `Hash`  | `(hash x) -> :number` | 哈希值 | `Hashable`   | `Hash`    |
| `Clone` | `(clone x) -> 'T`     | 深拷贝 | -            | `Clone`   |

**默认实现**: 所有不可变值类型自动满足（Calcit 默认 immutable）

#### 7. 默认值与转换 (Default & Conversion)

| Trait     | 方法签名                | 描述       | Haskell 对应 | Rust 对应   |
| --------- | ----------------------- | ---------- | ------------ | ----------- |
| `Default` | `(default T) -> 'T`     | 类型默认值 | `Default`    | `Default`   |
| `From`    | `(from T source) -> 'T` | 类型转换   | -            | `From/Into` |

**默认实现**:

- `Default`: `nil`→`nil`, `number`→`0`, `string`→`""`, `list`→`[]`, `map`→`{}`, `set`→`#{}`
- `From`: 常见转换如 `number->string`, `list->set`, `map->list`

### Trait 定义语法

```cirru
; 定义 Trait（方法签名声明）
deftrait Show
  :methods $ {}
    :show $ defn show (x) nil  ; nil 表示需要实现

; 定义 Trait（带默认实现）
deftrait Eq
  :methods $ {}
    :eq? $ defn eq? (x y) (&= x y)  ; 使用原始相等
    :not-eq? $ defn not-eq? (x y) (not (eq? x y))  ; 基于 eq? 的默认实现

; 带多个方法的 Trait
deftrait Compare
  :requires ([] Eq)  ; 依赖其他 Trait
  :methods $ {}
    :compare $ defn compare (x y) nil
    :lt? $ defn lt? (x y) (= :lt (compare x y))
    :gt? $ defn gt? (x y) (= :gt (compare x y))
    :lte? $ defn lte? (x y) (not (gt? x y))
    :gte? $ defn gte? (x y) (not (lt? x y))
```

### Trait 实现语法

使用 `with-traits` 函数式组合的方式为类型添加 Trait 实现：

```cirru
; 完整示例：为 Point 类型实现 Show 和 Eq trait
let
    ; 1. 定义基础 struct
    Point0 $ defstruct Point (:x :number) (:y :number)

    ShowTrait $ deftrait Show
      :show $ fn (x) |todo

    EqTrait $ deftrait Eq
      :eq? $ fn (x y)
        ; TODO
        do false

    ; 2. 定义 Show trait 的实现 (record 形式)
    show-impl $ %{} ShowTrait
      :show $ fn (p)
        str "|Point(" (.x p) ", " (.y p) ")"

    ; 3. 定义 Eq trait 的实现
    eq-impl $ %{} EqTrait
      :eq? $ fn (a b)
        and (= (:x a) (:x b)) (= (:y a) (:y b))

    ; 4. 使用 with-traits 组合，得到带 trait 实现的 struct
    Point $ with-traits Point0 show-impl eq-impl

    ; 5. 用 struct 创建 record 实例
    p1 $ %{} Point (:x 3) (:y 4)
    p2 $ %{} Point (:x 3) (:y 4)

  ; 6. 调用 trait 方法
  println (.show p1)        ; => "Point(3, 4)"
  println (.eq? p1 p2)      ; => true

; with-traits 可以接受多个 trait impl
; (with-traits struct-def impl1 impl2 impl3 ...)
```

### 分派规则

方法调用时的查找顺序：

1. **值自身的 classes** - `with-traits` 附加的实现（优先级最高）
2. **类型的内置 Trait 实现** - 如 `number` 自动拥有 `Add`
3. **calcit.core 的默认实现** - 兜底行为

```cirru
; 分派示例
.show 42        ; → 查找 number 的 Show 实现 → "42"
.show my-point  ; → 查找 Point record 的 Show 实现 → "Point(1, 2)"

; 显式 Trait 调用（消歧义）
Show/show 42
(trait-call Show :show 42)
```

### 内置类型的 Trait 实现映射

| 类型     | Show | Inspect | Eq  | Compare | Add | Multiply | Len | Foldable | Mappable | Hash |
| -------- | ---- | ------- | --- | ------- | --- | -------- | --- | -------- | -------- | ---- |
| `nil`    | ✓    | ✓       | ✓   | -       | -   | -        | -   | -        | -        | ✓    |
| `bool`   | ✓    | ✓       | ✓   | -       | -   | -        | -   | -        | -        | ✓    |
| `number` | ✓    | ✓       | ✓   | ✓       | ✓   | ✓        | -   | -        | -        | ✓    |
| `string` | ✓    | ✓       | ✓   | ✓       | ✓   | -        | ✓   | ✓        | ✓        | ✓    |
| `tag`    | ✓    | ✓       | ✓   | ✓       | -   | -        | -   | -        | -        | ✓    |
| `symbol` | ✓    | ✓       | ✓   | -       | -   | -        | -   | -        | -        | ✓    |
| `list`   | ✓    | ✓       | ✓   | ✓       | ✓   | -        | ✓   | ✓        | ✓        | ✓    |
| `map`    | ✓    | ✓       | ✓   | -       | -   | -        | ✓   | ✓        | ✓        | ✓    |
| `set`    | ✓    | ✓       | ✓   | -       | -   | -        | ✓   | ✓        | ✓        | ✓    |
| `tuple`  | ✓    | ✓       | ✓   | -       | -   | -        | ✓   | -        | -        | ✓    |
| `record` | ✓    | ✓       | ✓   | -       | -   | -        | ✓   | -        | -        | ✓    |
| `fn`     | ✓    | ✓       | -   | -       | -   | -        | -   | -        | -        | -    |

### 实施阶段

#### Phase 1: 基础 Trait 结构

1. ✅ 定义 `CalcitTrait` 数据结构 (src/calcit/calcit_trait.rs)
2. ✅ 在 `Calcit` enum 中添加 `Trait(CalcitTrait)` 变体
3. 在 `calcit.core` 中定义 `Show`, `Eq`, `Add`, `Multiply`
4. 内置类型自动拥有对应实现
5. `invoke_method` 支持 Trait 方法查找

#### Phase 2: 语法支持

1. `deftrait` 宏定义
2. `defimpl` 宏定义
3. `assert-trait` 运行时检查
4. Trait 方法的显式调用语法

#### Phase 3: 扩展

1. 完整的集合 Trait (`Foldable`, `Mappable`, `Filterable`)
2. `Default`, `From` 转换 Trait
3. Trait 继承/组合
4. 更好的错误消息

---

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
