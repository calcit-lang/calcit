# Runtime Traits for Calcit (Draft)

> 目标：在运行时提供一套 Trait 机制，即使无法静态检查，也能把错误前移到调用点，并允许类型级别的能力声明（如 `Show`、`Add`、`Multiply` 等）。

> 文档说明（更新于 2026-02-08）：本文件用于“解释当前实现 + 修正计划”。其中“当前实现/分派规则”以现有 Rust runtime / preprocess / JS backend 的真实行为为准；计划部分提供分阶段目标与验收口径。

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

| Trait      | 方法签名                | 描述       | Haskell 对应 | Rust 对应        | 备注                                 |
| ---------- | ----------------------- | ---------- | ------------ | ---------------- | ------------------------------------ |
| `Foldable` | `(fold x init f) -> 'A` | 折叠/归约  | `Foldable`   | `Iterator::fold` | ✅ 命名来自 Haskell                  |
| `Functor`  | `(fmap x f) -> 'T`      | 保结构映射 | `Functor`    | `Iterator::map`  | 🎯 **改名建议**：用 Haskell 正统命名 |
| `Iterable` | `(iter x) -> iterator`  | 获取迭代器 | -            | `IntoIterator`   | 统一迭代抽象                         |

**动态语言务实方案（可选）：**

- 不单独定义 `Functor`/`Filterable`，直接在集合类型上提供 `.map`/`.filter` 方法
- 或统一为 `Collection` trait，包含 `map`, `filter`, `fold` 全套操作
- 类似 Rust 的 `Iterator` 或 JavaScript 的 `Array` 方法

**默认实现**: `list`, `map`, `set`, `string`

**设计权衡说明：**

- Haskell 的 `Functor`/`Monad` 威力在于类型系统保证代数定律
- 动态语言无法静态检查这些定律，过度抽象可能适得其反
- 推荐：借鉴概念，但保持实用导向（用户更关心"能不能 map"，而非"是不是 Functor"）

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

### Trait 定义语法（当前实现）

当前 `deftrait` 宏仅用于声明 trait 名称与方法列表，内部调用 `&trait::new`。

```cirru
deftrait Show :show
deftrait Eq :eq?
deftrait Compare :compare
```

> 说明：默认实现、依赖与方法签名描述仍在规划中，尚未落地。

### Trait 实现语法（当前实现）

使用 `impl-traits` 函数式组合的方式为类型添加 Trait 实现（当前仍以 record 承载 impl，计划升级为独立 impl 结构，见后文）：

```cirru
; 完整示例：为 Point 类型实现 Show 和 Eq trait
let
    ; 1. 定义基础 struct
    Point0 $ defstruct Point (:x :number) (:y :number)

    ShowTrait $ deftrait Show
    EqTrait $ deftrait Eq

    ; 2. 定义 Show trait 的实现 (record 形式)
    show-impl $ %{} ShowTrait
      :show $ fn (p)
        str "|Point(" (.x p) ", " (.y p) ")"

    ; 3. 定义 Eq trait 的实现
    eq-impl $ %{} EqTrait
      :eq? $ fn (a b)
        and (= (:x a) (:x b)) (= (:y a) (:y b))

    ; 4. 使用 impl-traits 组合，得到带 trait 实现的 struct
    Point $ impl-traits Point0 show-impl eq-impl

    ; 5. 用 struct 创建 record 实例
    p1 $ %{} Point (:x 3) (:y 4)
    p2 $ %{} Point (:x 3) (:y 4)

  ; 6. 调用 trait 方法
  println (.show p1)        ; => "Point(3, 4)"
  println (.eq? p1 p2)      ; => true

; impl-traits 可以接受多个 trait impl
; (impl-traits struct-def impl1 impl2 impl3 ...)
```

### Trait 约束与标注（新增）

`assert-traits` 用于在编译期提供“trait 标注”，并在运行时进行检查：

- **编译期标记**：将本地变量标注为 trait 类型，供方法解析与类型提示使用。
- **运行时检查**：确保值确实实现该 trait（缺失方法时直接报错）。

补充（与当前实现一致）：

- `assert-traits` 是一个语法（syntax），会在 preprocess 阶段被展开为对内建过程 `&assert-traits` 的调用；runtime 并不直接执行 `assert-traits` 语法本身。
- 当前实现要求第一个参数必须是 **local**，用于把 type-info 写回作用域类型表并用于后续方法校验/类型提示。

**行为细则（基于当前实现，补充更具体的定义）：**

1. **两种写法与作用范围**

- **函数 body 顶层写法**（推荐，作为函数前置约束）：
  - 写在函数 body 顶层的 `assert-traits x Trait` 被视为“参数/局部的全局约束”。
  - 静态分析会把它当作参数类型提示来源（与 `assert-type` 同级别），影响整个函数体内的 `.method` 校验与提示。
  - 运行时会在执行到该表达式时进行检查（缺失方法则抛错）。

- **函数 body 内部嵌套写法**（表达式/返回值约束）：
  - `assert-traits` 作为表达式参与计算时，主要用于约束该表达式（及其返回值链路）；适用于在局部链路上增加约束，减少 `let _ ...` 这类“仅为检查而写”的绑定。
  - 当前实现的参数类型提示扫描会遍历函数体中的 `assert-traits`，因此严格来说嵌套写法也可能被识别为参数提示；但约定上仅将顶层写法视为“函数级前置约束”。

2. **静态分析（preprocess）行为**

- 会在当前作用域把 local 的 type-info 写入 `scope_types`，供后续 `.method` 校验/补全使用。
- `assert-traits` 参数中的 trait 解析规则：
  - 能解析为 trait 定义的，进入 trait 集合。
  - 若解析不到 trait，会降级为自定义类型标注，仅用于类型提示（不参与方法校验）。
- 多个 trait 通过 **append** 形成 `TraitSet`；当多次 `assert-traits` 作用于同一个 local 时，后者的 type-info 会覆盖前者。

3. **运行时行为**

- 预处理会把 `assert-traits` 展开成一连串 `&assert-traits` 调用，运行时逐个检查 trait 是否实现。
- 检查顺序按写入顺序执行；当 trait 列表存在重复方法时，实际方法分派仍遵循 impl 分派规则（user impls last-wins / core impls first-wins）。
- **内置类型限制**：list/map/set/string/number 等内置数据结构的 impl 列表是固定的，`assert-traits` **不会** 在运行时扩充它们的可用方法，只做“默认实现是否存在”的校验。
- **静态/运行时不一致**：由于静态分析信息有限，preprocess 的 trait 标注可能比运行时更宽松；当前允许这种不一致存在。

4. **顺序与覆盖**

- trait 信息的记录是 **append 模式**，但对方法分派的“命中优先级”仍以 impl 链为准；当存在多重 trait/impl 时，**从末尾开始命中**（last-wins）是用户自定义 impl 的生效方向。
- `assert-traits` 只影响“能不能调用/能否单态化”的判断，不改变实际方法分派的实现来源。

```cirru
; 在调用点标注并断言 trait 能力
assert-traits x Show
; 允许编译期把 x 视作 trait object，方便后续的 .show/.eq? 等方法校验
```

**类型标注支持**：trait 定义可作为类型标注值使用（与 struct/enum 类似）。

- 例：`assert-traits x Show` 会把 `x` 的类型标注为 `trait Show`。

### 分派规则（当前实现）

当前 `.method` 调用（包含 trait 方法）本质上是对“impl records 列表”的查找。

分派来源与优先级：

| 场景         | impl 来源                                                        | 覆盖策略       | 扫描方向 | 备注                                                         |
| ------------ | ---------------------------------------------------------------- | -------------- | -------- | ------------------------------------------------------------ |
| 用户自定义值 | Record/Tuple/Struct/Enum 实例的 `impls`（由 `impl-traits` 追加） | **last-wins**  | 从尾到头 | 支持“后追加覆盖前实现”（append-to-override）                 |
| 内置类型     | `calcit.core` 中的 `&core-*-impls`（core impl list）             | **first-wins** | 从头到尾 | 保持 core 列表顺序语义（如 list `.add` vs Add trait `:add`） |

1. **用户自定义值的 `impls`（Record/Tuple/Struct/Enum 实例）**

- `impl-traits` 会把新的 impl record **追加** 到值的 `impls` 末尾（不可变地返回新值）。
- 查找策略：**last-wins**（从尾到头扫描，先命中先调用）。
- 目的：支持稳定的“后追加覆盖前已有实现”的工作流。

2. **内置类型的 core impl 列表（list/map/number/string/set/fn 等）**

- runtime 会把值映射到 `calcit.core` 中的 `&core-*-impls`（这是一个 record 或 record 列表）。
- 查找策略：**first-wins**（从头到尾扫描，先命中先调用）。
- 目的：保持 `calcit.core` 中现有 impl 列表的顺序语义（已存在依赖顺序的案例：list 的 `.add` 与 Add trait 的 `:add` 同名时，core 列表顺序决定行为）。

一致性要求：preprocess 的方法校验/内联与 JS backend 的 `invoke_method` 需要与上述规则保持一致（当前已对齐）。

```cirru
; 分派示例
.show 42        ; → 查找 number 的 Show 实现 → "42"
.show my-point  ; → 查找 Point record 的 Show 实现 → "Point(1, 2)"

; 显式 Trait 调用（计划，用于消歧义）
; (trait-call Show :show 42)
; (Show/show 42)
```

### 内置类型的 Trait 实现映射

- `nil`: Show, Inspect, Eq, Hash
- `bool`: Show, Inspect, Eq, Hash
- `number`: Show, Inspect, Eq, Compare, Add, Multiply, Hash
- `string`: Show, Inspect, Eq, Compare, Add, Len, Foldable, Functor, Hash
- `tag`: Show, Inspect, Eq, Compare, Hash
- `symbol`: Show, Inspect, Eq, Hash
- `list`: Show, Inspect, Eq, Compare, Add, Len, Foldable, Functor, Hash
- `map`: Show, Inspect, Eq, Len, Foldable, Functor, Hash
- `set`: Show, Inspect, Eq, Len, Foldable, Functor, Hash
- `tuple`: Show, Inspect, Eq, Len, Hash
- `record`: Show, Inspect, Eq, Len, Hash
- `fn`: Show, Inspect

### 实施阶段（对照当前进度）

#### Phase 1: 基础 Trait 结构 ✅ **已完成**

1. ✅ 定义 `CalcitTrait` 数据结构 (src/calcit/calcit_trait.rs)
2. ✅ 在 `Calcit` enum 中添加 `Trait(CalcitTrait)` 变体
3. ✅ 在 `calcit.core` 中定义 `Show`, `Eq`, `Add`, `Multiply`, `Len` 等核心 trait
4. ✅ 内置类型自动拥有对应实现（已在运行时与 JS backend 对齐）
5. ✅ `invoke_method` 支持 Trait 方法查找
6. ✅ 从 `class` 系统迁移到 `trait` 和 `impls` (commit 73aa249)
7. ✅ `impl-traits` 函数支持为 record/tuple/struct/enum 追加 impl
8. ✅ JS backend 完整支持 CalcitTrait 及相关操作
9. ✅ 移动内部实现到 `calcit.internal` 命名空间以清理代码结构

#### Phase 2: 语法支持 🔄 **部分完成**

1. ✅ `deftrait` 宏定义（支持方法名 + 类型签名）
2. ✅ 基础 trait 实现语法（通过 record + `impl-traits`）
3. ✅ 测试覆盖：`test-traits.cirru` 包含 Show/Eq/Compare/Add/Len 等基础测试
4. ✅ `defimpl` 最小宏定义（当前展开为 `defrecord!`，用于更清晰地定义 impl record）
5. ⏳ `defimpl ... for ...`（让宏直接表达“trait + 目标类型/值”，并减少手写 `impl-traits` 的样板）
6. ✅ `assert-traits` 运行时检查与编译期标注（当前限制：第一个参数需要是 local；preprocess 展开为 `&assert-traits`）
7. ⏳ Trait 方法的显式调用语法（如 `Show/show` 或 `trait-call`）

#### Phase 3: 扩展 📋 **待实现**

1. ⏳ 完整的集合 Trait (`Foldable`, `Mappable`, `Filterable`)
2. ⏳ `Default`, `From` 转换 Trait
3. ⏳ Trait 依赖（`requires`）与默认实现（`defaults`）
4. ⏳ Trait 继承/组合
5. ⏳ 更好的错误消息（trait 不匹配时的详细提示）
6. ⏳ 性能优化（trait 查找缓存）

---

## 范围评估

### 1) 核心数据结构与运行时

- **`Calcit` 新增变种**：`Trait`（已完成），新增 `Impl` 变种用于承载 trait impl（从 record 迁移）。
- **运行时上下文**：不单独引入 registry，内置 Trait 直接放在 `calcit.core`，其他 Trait 作为普通值附着在各自命名空间上。
- **调用分派**：方法调用/操作符调用在当前命名空间与 `calcit.core` 中解析 Trait 值，再执行绑定实现。
- **Trait 实现承载**：Traits 以组合方式使用，携带实现时统一使用 `Vec`；计划引入 `CalcitImpl` 作为一等结构，替代 record 承载（同时保留向后兼容的过渡层）。
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
- **Trait 约束表达**：考虑引入 `assert-traits`，用法类似 `assert-type`，用于声明约束与在调用处触发运行时检查。

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

- `Calcit` 匹配逻辑：新增 `Impl` 变种会导致编译错误与运行时路径调整（允许少量 breaking）。
- 内建操作行为：如果 `+/*` 改为 trait 分派，某些动态调用会改变错误时机。
- `method` 分派：同名方法在不同 impl 记录之间的覆盖策略会改变行为（当前已确定规则，见“分派规则”）。
- `str`/`format` 与 `Show` 的统一：输出可能略有不同。

## 设计决策待确认

1. Trait 分派优先级与覆盖策略：

- 已确定：用户自定义值 `impls` 采用 last-wins；内置类型 core impl 列表采用 first-wins。
- 待补：提供显式调用语法（`trait-call` / `Show/show`）用于消歧义，以及冲突时的告警/错误策略。

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
- 迁移 trait impl 承载：record -> `CalcitImpl`（保留兼容路径，逐步淘汰 record impl）。
- `assert-traits` / `requires` 运行时检查。

### Phase 3：扩展与稳定

- 覆盖更多内建类型能力。
- 增加 tests（cirru 文件）覆盖 trait 注册、冲突、失败路径。

---

## 当前实现要点（补充）

- `deftrait` 已存在，展开为 `&trait::new`。
- `defimpl` 已存在（最小实现）：展开为 `defrecord!`，用于更清晰地定义“trait impl record”（计划引入 `CalcitImpl` 后更新宏展开目标）。
- `impl-traits` 已在 Rust 与 JS backend 支持，可对 record/tuple/struct/enum 追加 impl。
- JS 侧已补齐 `CalcitTrait` 类型、`type-of`、`toString` 与 `&trait::new`、`&record:impl-traits` 等对应实现。
- `.method` 分派采用“混合优先级”：用户自定义值（record/tuple/struct/enum 实例）为 last-wins，内置类型 core impl 列表为 first-wins。
- `assert-traits` 已作为 preprocess 语法落地：写入 local 的 type-info，并展开为 `&assert-traits` 做运行时检查（当前限制：第一个参数需要是 local）。

---

## 近期执行计划（从简单稳定开始）

> 目标：先把行为与回归边界钉死，再推进语言层能力（`defimpl` / 显式 trait-call）。

1. **清理与基线（半天内）**

- [ ] 把这轮 traits 改动按边界整理成 3 组：语义实现 / 测试 / 文档（便于 review）。
- [x] 检查是否有生成物被误纳入改动（如 `js-out/` 等）：当前工作区改动仅包含计划/测试/core，未发现产物噪音需要纳入跟踪。

2. **固化“分派优先级”规范 + 回归测试（1 天）**

- [x] 文档固化规则：builtin core impl list = first-wins；record/tuple impls = last-wins（append-to-override）。
- [x] 回归：core list 的 `.add` 不被 Add trait `:add` 覆盖（靠 core 列表顺序 + first-wins）。
- [x] 回归：record 上 `impl-traits` 追加实现能稳定覆盖旧实现（last-wins）。
- [x] 回归：tuple 上 `impl-traits` 覆盖链同样稳定（last-wins）。
- [x] 再补 1 个“更尖锐”的 Rust/JS 共用断言：不同 trait 提供同名方法时，仍以 `impl-traits` 追加顺序决定命中（覆盖 record + tuple）。

3. **动态 trait 调用告警（1 天）**

- [x] 引入 `--check-dyn-trait`：在 `cr` 正常执行流程中，对无法单态化的 `.method` 调用给出 warning。
- [x] 规则：当 receiver 无法解析到具体 impl（类型为 `:dynamic`/未知），且未被 `assert-traits` 标注时触发告警。
- [x] 验收：warning 包含方法名与位置；`assert-traits` 后 warning 消失；Rust/JS 预处理行为一致。

4. **补齐语言层能力：`defimpl` + 显式 trait-call（2～4 天）**

- [x] `defimpl`（v0）：让“写 impl record”有标准入口，减少手写 `defrecord!` 的样板。
- [ ] `defimpl ... for ...`：让宏直接表达“trait + 目标类型/值”，并自动完成挂载（减少手写 `impl-traits`）。
- [ ] 显式 trait-call（如 `Show/show` 或 `trait-call`）：提供绕开 `.method` 分派的稳定通道，便于 debug override 链。
- [ ] 验收：同一段代码在 preprocess 校验、Rust runtime、JS runtime 行为一致。

5. **`assert-traits` 易用性补强（可选，1～2 天）**

- [ ] 支持 `assert-traits` 的第一个参数是任意表达式：preprocess 自动提升为临时 local 再做检查（纯语法糖）。
- [ ] 验收：`assert-traits (+ 1 2) ...` 可用，且错误位置/提示依然清晰。

6. **冲突策略与可观测性（可选，2～3 天）**

- [ ] 提供“冲突诊断”开关：当同名方法多次出现时，打印候选列表/命中来源（对 last-wins 特别有帮助）。
- [x] 可选 debug proc：`&methods-of` / `&inspect-methods`，返回/打印某值当前 impl 链与可用方法集合，帮助定位“为什么命中这个实现”。

---

## Checklist（后续跟踪）

### 🎯 推荐优先实现（短期，1-2周）

**理由：完善当前已有的 trait 机制，提升用户体验**

- [ ] **`defimpl` 宏**：简化 trait 实现语法
  - 当前（已实现 v0）：`defimpl MyTrait MyImpl (:method value) ...`（展开为 `defrecord!`，产出“impl record”）
  - 仍待补齐：`defimpl MyTrait for MyType ...`（自动挂载/注册，减少显式 `impl-traits`）
  - 优势：语义更清晰，自动完成 impl-traits 步骤
  - 验收：宏展开在 Rust/JS 下语义一致；错误信息包含 trait/类型/缺失方法；`test-traits.cirru` 覆盖正常/冲突路径
- [ ] **显式 trait 调用语法**：解决方法名冲突
  - 语法选项：`(trait-call Show :show x)` 或 `(Show/show x)`
  - 用例：当一个类型实现多个 trait，且方法名冲突时
  - 验收：可在运行时绕开 `.method` 的歧义（不受 impl 覆盖影响）；preprocess 能做方法存在性与参数数量/类型校验；Rust/JS 行为一致
- [x] **`assert-traits` 运行时检查**：前移错误发现时机（已实现；当前限制：第一个参数需要是 local）
  - 语法：`(assert-traits x Show)` 或 `(requires x Show)`
  - 在函数入口检查参数是否满足 trait 约束
  - 提供清晰的错误消息
  - 下一步：扩展到函数参数/模式绑定等更多场景（不止 local）

### 🔧 中期实现（3-4周）

**理由：扩展 trait 系统能力，支持更复杂的场景**

- [ ] **Trait 依赖（`requires`）**：声明 trait 之间的依赖关系
  - 例：`Ord` 依赖 `Eq`
  - 实现时自动检查依赖是否满足
- [ ] **默认实现（`defaults`）**：减少重复代码
  - 在 trait 定义中提供默认方法实现
  - 类型可以选择性覆盖
- [ ] **完整的集合 Trait（重新评估设计）**：
  - **方案 A（Haskell 风格）**：独立 `Functor`/`Foldable` trait
    - 优点：概念纯粹，严格分离关注点
    - 缺点：在动态语言中过度抽象，用户学习成本高
  - **方案 B（Rust/JS 风格）**：统一 `Collection` trait
    - 包含 `map`, `filter`, `fold`, `count`, `empty?` 等全套操作
    - 优点：实用导向，一站式接口
    - 缺点：trait 体积大，部分类型可能只能实现子集
  - **方案 C（混合）**：保留 `Foldable` 基础，`map`/`filter` 作为可选扩展
    - 最小公约数是 `fold`（可实现 map/filter）
    - 类型按需实现 map/filter 优化版本
  - 🎯 **推荐**：先实现方案 B（务实路线），观察实际使用后再考虑拆分

- [ ] **冲突检测与覆盖策略**：
  - 同一类型多 impl 时的优先级规则
  - 重复注册时的警告机制

**Functor/Monad 补充说明：**

- 在 Calcit 这样的动态语言中，Monad 的核心价值（`>>=` 的类型组合）基本丧失
- 但 `Functor` (fmap) 仍有意义：统一"保结构变换"的概念
- 实际实现时可考虑：
  - ✅ 提供 `fmap` 作为标准方法名（对 FP 用户友好）
  - ✅ 同时保留 `.map` 别名（对主流用户友好）
  - ❌ 不强制实现完整 Monad（`return`/`>>=` 在无类型约束时意义不大）

### 🚀 长期规划（1-2月）

**理由：提升系统稳定性和性能**

- [ ] **转换 Trait (`Default`, `From`)**：
  - 类型间的标准转换接口
  - 减少手写转换函数
- [ ] **Trait 继承/组合**：
  - 支持 trait 继承（如 `trait Ord extends Eq`）
  - 或 trait 组合（如 `trait Num = Add + Multiply + ...`）
- [ ] **性能优化**：
  - Trait 查找缓存（避免重复遍历）
  - 内联常见 trait 方法（如 `show`, `eq?`）
- [ ] **更好的错误消息**：
  - Trait 不匹配时显示期望 vs 实际
  - 建议可能的解决方案
- [ ] **文档与示例**：
  - 完整的 trait 使用指南
  - 常见模式与最佳实践
  - 更多 `test-traits.cirru` 测试用例

### 📝 技术债务清理

- [x] ~~从 `class` 迁移到 `trait`~~ (已完成，commit 73aa249)
- [x] ~~移动内部函数到 `calcit.internal`~~ (已完成，commit fc78725)
- [ ] 解决 JS 编译模式的循环依赖问题
  - 当前问题：`calcit.internal.mjs` 引用 `calcit.core.mjs` 导致初始化失败
  - 可能方案：调整模块加载顺序或使用延迟初始化

---

## 实施建议

**下一步行动（按优先级）：**

1. **先做（低风险）**：清理基线 + 补齐“分派优先级”尖锐回归

- 把行为边界钉死，避免后续 `defimpl` / trait-call 引入难定位回归

2. **然后（核心能力）**：`defimpl` 宏

- 降低写 impl 的样板，提高可读性

3. **再做（可观测性/可维护）**：显式 trait 调用语法

- 解决方法名冲突与 override 链难 debug 的实际问题
- 为后续 trait 组合打基础

3. **然后**：Trait 依赖 + 默认实现
   - 这是更复杂的功能，依赖前面的基础
   - 可以大幅减少样板代码

4. **最后**：集合 Trait + 性能优化
   - 在系统稳定后进行性能调优
   - 逐步扩展 trait 覆盖范围

---

## 原有 Checklist（归档）

以下是原计划中的项目，已整合到上面的分类中（按真实实现状态更新）：

- [ ] `defimpl ... for ...`（包含方法名校验/去重规则）→ 短期优先（注：defimpl v0 已存在，仅产出 impl record）
- [x] `assert-traits` 运行时检查与错误消息格式（已实现；当前限制：第一个参数需要是 local）
- [ ] 显式 trait 调用语法（`trait-call` / `Show/show` 语法）→ 短期优先
- [ ] trait 依赖（`requires`）与默认实现（`defaults`）的表达与存储 → 中期实现
- [ ] 统一 `Compare` 的三态返回与 `&compare` 的关系（`<`/`>` 仅数字）→ 中期实现
- [ ] 冲突检测：同一对象多 impl 的覆盖顺序与警告策略 → 中期实现
- [x] ~~JS backend 与 Rust 行为一致性验证（新增 tests）~~ → 已有 test-traits.cirru
- [ ] 文档示例与 `test-traits.cirru` 覆盖更多失败路径 → 长期规划

## 测试补充建议（cirru）

- `test-traits.cirru`：
  - `Show` for number/string/list/map
  - `Add`/`Multiply` for number
  - 未实现时的错误提示
  - 多实现冲突/重复注册

---

> 备注：该方案涉及 runtime/stdlib/codegen 全链路改造，建议先从最小可运行集开始迭代。
