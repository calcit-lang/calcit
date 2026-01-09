# Calcit 局部变量类型标记方案评估

本文档评估在 Calcit 中为 `Local` 变量补充类型信息的技术方案及工作量。

## 1. 核心目标

- 在宏展开后的 IR (Intermediate Representation) 中，为 `Local` 变量关联类型信息。
- 支持 `assert-type` 和 `hint-fn` 语法进行函数参数、返回值类型标记。
- 自动利用 `Record` 信息进行方法调用（Method Call）的静态验证。
- 允许 `unknown` 类型，并提供运行时查看手段。
- 为内置函数补充类型提示，保持向后兼容。

## 2. 技术方案建议

### 2.0 类型声明语法设计

Calcit 以前没有预置函数类型写法，现通过 `assert-type` 和 `hint-fn` 来声明类型信息：

#### 2.0.1 基本类型声明

```cirru
defn f1 (x y)
  assert-type x :number                    ; 声明参数 x 为 number 类型
  assert-type y $ :fn (:number) :number    ; 声明参数 y 为函数类型
  hint-fn $ return-type :number             ; 声明返回值类型
  &+ x (y 10)
```

#### 2.0.2 类型表示方式

- **内置类型**：使用 Tag 直接表示

  - `:number` - 数字类型
  - `:string` - 字符串类型
  - `:bool` - 布尔类型
  - `:nil` - nil 类型
  - `:keyword` - 关键字类型
  - `:fn` - 函数类型（基础形式）
  - `:fn (arg-types...) return-type` - 函数类型（带签名）

- **自定义类型**：使用 Record 定义来表示

  - `:User` - 当前 namespace 中定义的 Record
  - 类型信息通过 `defrecord` 关联的元数据获取

- **复合类型**：
  - `:list :number` - 元素为 number 的 list
  - `:map :keyword :string` - key 为 keyword，value 为 string 的 map

#### 2.0.3 `assert-type` 语法

`assert-type` 用于在函数体内声明变量类型：

```cirru
assert-type <variable> <type-expr>
```

- `<variable>`：要标注类型的变量名（Symbol）
- `<type-expr>`：类型表达式，可以是 Tag 或嵌套的列表结构

示例：

```cirru
defn process-user (user)
  assert-type user :User       ; user 是 User Record 类型
  get user :name               ; 可以静态检查字段是否存在

defn map-numbers (f xs)
  assert-type f $ :fn (:number) :number    ; f 是函数类型
  assert-type xs $ :list :number            ; xs 是 number 列表
  map f xs
```

#### 2.0.4 `hint-fn` 语法

`hint-fn` 用于在函数体内声明函数级别的元信息（如返回值类型）：

```cirru
hint-fn $ return-type <type-expr>
```

示例：

```cirru
defn add-numbers (a b)
  assert-type a :number
  assert-type b :number
  hint-fn $ return-type :number
  &+ a b
```

#### 2.0.5 类型传播机制

1. `assert-type` 将类型信息注入到 `ScopeTypes` 映射中
2. 预处理器在转换 `Symbol` → `Local` 时查询并填充 `type_info` 字段
3. 类型信息在作用域内传播，后续对同一变量的引用保留类型标注
4. `hint-fn` 的返回值类型信息存储在函数定义的元数据中

#### 2.0.6 未标注类型的处理

- 默认所有未标注的变量 `type_info` 为 `None`，对应 `unknown` 类型
- `unknown` 类型不触发静态类型检查
- 保持向后兼容：老代码无需修改即可运行

### 2.1 数据结构调整

#### 2.1.1 `CalcitLocal` 扩展

在 `src/calcit/local.rs` 中修改 `CalcitLocal` 结构体：

```rust
pub struct CalcitLocal {
  pub idx: u16,
  pub sym: Arc<str>,
  pub info: Arc<CalcitSymbolInfo>,
  pub location: Option<Arc<Vec<u16>>>,
  // 新增：类型信息，可以是 Tag, Record 或其它 Calcit 值
  pub type_info: Option<Arc<Calcit>>,
}
```

#### 2.1.2 函数定义元数据扩展

为支持 `hint-fn` 声明的返回值类型，需要在函数相关结构中添加类型元数据字段：

```rust
// 在函数定义相关结构中添加（具体位置取决于实现）
pub struct FnInfo {
  // ... 现有字段
  pub return_type: Option<Arc<Calcit>>,  // 返回值类型
  pub arg_types: Vec<Option<Arc<Calcit>>>, // 参数类型列表
}
```

- `return_type`: 通过 `hint-fn $ return-type :type` 声明
- `arg_types`: 通过 `assert-type arg :type` 在函数体内收集
- 默认值为 `None`，表示 `unknown` 类型

### 2.2 预处理器增强 (`preprocess.rs`)

预处理器是类型信息注入的关键位置：

1. **作用域跟踪**：`preprocess_expr` 需要传递一个类型映射表 `ScopeTypes = HashMap<Arc<str>, Arc<Calcit>>`。

2. **`assert-type` 处理**：

   - 识别 `assert-type var type` 语法节点
   - 解析 `var` 对应的变量名，将 `type` 表达式求值并记录到 `ScopeTypes` 映射中
   - `assert-type` 在预处理后可以作为 No-op 或保留作为运行时断言

3. **`hint-fn` 处理**：

   - 识别 `hint-fn $ return-type type` 语法
   - 将返回值类型信息关联到当前函数定义的元数据中
   - 可扩展支持其他函数级别的提示（如 `hint-fn $ pure true`）

4. **Local 生成**：

   - 当预处理器将 `Symbol` 转换为 `Local` 时，从 `ScopeTypes` 中查询并填充 `type_info`
   - 确保类型信息在作用域内正确传播

5. **方法校验**：
   - 在 `preprocess_list_call` 中，识别 `get`/`.-field` 等方法调用
   - 如果参数是带有 `Record` 类型的 `Local`，根据 Record 定义校验字段合法性
   - 不匹配时生成 `LocatedWarning`

### 2.3 内置函数与类型提示

- **Procs 注册**：在 `src/builtins.rs` 中，需要一种方式为 `CalcitProc` 关联签名信息。
- **兼容性**：老代码保持 `type_info` 为 `None` (即 `unknown`)，不进行强校验。

### 2.4 运行时支持

- 增加 `&inspect-type` 原语，允许在运行时通过 `CalcitLocal` 获取其标记的类型名。

## 3. 工作量评估

| 模块                | 任务描述                                                         | 预计工时 (天) |
| :------------------ | :--------------------------------------------------------------- | :------------ |
| **基础架构**        | 修改 `CalcitLocal` 结构及相关的 `Hash`/`Ord`/`Eq` 实现           | 1             |
| **核心语法**        | 添加 `assert-type` 和 `hint-fn` 到 `CalcitSyntax` 并更新解析逻辑 | 1.5           |
| **预处理器 (核心)** | 实现基于作用域的类型跟踪，处理类型标记注入                       | 5             |
| **Record 分析**     | 实现方法调用与 Record 字段的静态匹配校验                         | 2             |
| **内置函数适配**    | 建立内置函数类型描述体系，适配常用核心函数                       | 3             |
| **调试工具**        | 实现 `inspect-type` 及相关的错误信息增强                         | 1             |
| **测试与文档**      | 编写测试用例验证类型传播及边界情况                               | 2             |
| **总计**            |                                                                  | **约 15 天**  |

## 4. 风险与考量

- **宏展开顺序**：由于类型标记可能出现在宏展开之后，预处理器必须确保在所有宏完全展开后仍能正确关联类型。
- **性能开销**：在预处理阶段增加哈希查找和类型比对会有一定的编译性能损耗。
- **复杂类型**：目前方案主要针对 `Record` 和基础类型，暂不考虑泛型或复杂的联合类型，以降低实现难度。

## 5. 阶段性开发计划

| 阶段                         | 目标                                                                                        | 交付物                                                             |
| :--------------------------- | :------------------------------------------------------------------------------------------ | :----------------------------------------------------------------- |
| **阶段 1：数据结构准备**     | 扩展 `CalcitLocal` 结构与派生实现，预留 `type_info` 字段，加入 `assert-type` 语法占位。     | 可编译版本 + 基础单测覆盖 `CalcitLocal` 序列化/比较逻辑。          |
| **阶段 2：语法解析**         | 在 Cirru→Calcit 转换以及预处理器中识别 `assert-type` 和 `hint-fn`，能够在 AST 中保留注解。  | `cr --check-only` 样例，通过 Cirru 示例展示 `assert-type` 表达式。 |
| **阶段 3：类型传播**         | 在 `preprocess_defn`/`preprocess_core_let` 等处传递类型映射，`Local` 节点带上 `type_info`。 | demo 函数 + `&inspect-type` 临时 helper，证明类型信息可查询。      |
| **阶段 4：Record 静态校验**  | 针对方法调用与 `Record` 类型做字段校验，错误信息可定位。                                    | 合法/非法调用示例、`cr query error` 输出截图。                     |
| **阶段 5：运行时与内置函数** | 提供 `&inspect-type`、补充部分内置函数签名，确保未标注时回退 `unknown`。                    | 运行时示例 + 文档更新 + 额外测试。                                 |

每个阶段结束后都需提交一份可正常编译的版本，并附上对应 Cirru 示例或 Rust 测试输出，方便验收。

**阶段进展（截至 2026-01-08 晚）**

- ✅ **阶段 1**：`CalcitLocal` 和 `CalcitFn` 结构已扩展，所有必要字段添加完成；`cargo test` 通过。
- ✅ **阶段 2**：语法解析完成
  - ✅ `assert-type` 语法已正确识别和解析
  - ✅ `hint-fn` 语法已识别（运行时为 no-op）
  - ✅ 测试用例已更新：`parses_assert_type_list`、`passes_assert_type_through_preprocess`
- ✅ **阶段 3**：类型传播完成
  - ✅ 预处理器维护局部类型映射（`ScopeTypes`）
  - ✅ `assert-type` 在预处理后返回 `Nil`，类型信息注入到 `Local.type_info` 和 `ScopeTypes`
  - ✅ 类型信息在作用域内正确传播
  - ✅ 测试验证通过：`propagates_type_info_across_scope`
  - ✅ 实际 Cirru 代码测试通过（`calcit/test-types.cirru`）
- 🚧 **阶段 4**：Record/Enum 静态校验部分完成
  - ✅ 实现了 `&record:get` 字段验证
  - ✅ 实现了 `.-field` 方法访问语法的字段验证
  - ✅ 单元测试验证通过：`validates_record_field_access`、`warns_on_invalid_record_field`、`validates_method_field_access`、`warns_on_invalid_method_field_access`
  - ⚠️ **当前限制**：只支持基于 `ScopeTypes` 的静态验证，需要在代码中显式使用 `assert-type` 标注
  - ⏸️ 待完成：基于 Record 字面量推断类型
  - ⏸️ 待完成：Enum 变体验证

### 当前实现状态说明（2026-01-09 更新）

**已完成的核心功能：**

1. ✅ `CalcitLocal.type_info: Option<Arc<Calcit>>` - 变量类型信息存储
2. ✅ `CalcitFn.return_type` 和 `CalcitFn.arg_types` - 函数类型元数据
3. ✅ `assert-type var type` 语法完整实现：
   - Cirru 解析正确识别
   - 预处理器将类型注入 `ScopeTypes` 和 `Local.type_info`
   - 预处理后返回 `Nil`（运行时无影响）
4. ✅ `hint-fn $ return-type :type` 语法识别（运行时 no-op）
5. ✅ 类型信息在作用域内传播，后续引用保留类型标注
6. ✅ 所有单元测试通过（11 个测试）
7. ✅ 实际 Cirru 代码可以使用类型标注并正常运行
8. ✅ **Record 字段验证**：
   - ✅ 在预处理阶段检查 `&record:get` 调用
   - ✅ 在预处理阶段检查 `.-field` 方法访问语法（如 `user.-name`）
   - ✅ 验证字段名是否存在于 Record 类型定义中
   - ✅ 生成编译时警告提示字段错误

**测试覆盖：**

- `calcit/test-types.cirru` - 包含多种类型标注场景的实际代码测试
- 单元测试验证类型解析、传播和作用域管理
- Record 字段验证单元测试（4 个）：
  - `validates_record_field_access` - `&record:get` 正确字段
  - `warns_on_invalid_record_field` - `&record:get` 错误字段
  - `validates_method_field_access` - `.-field` 正确字段
  - `warns_on_invalid_method_field_access` - `.-field` 错误字段

**Record 字段验证示例：**

```rust
// 单元测试中的示例
let test_record = Calcit::Record(CalcitRecord {
  name: EdnTag::from("Person"),
  fields: Arc::new(vec![EdnTag::from("age"), EdnTag::from("name")]),
  values: Arc::new(vec![Calcit::Nil, Calcit::Nil]),
  class: None,
});

// 在 scope_types 中注册类型
scope_types.insert(Arc::from("user"), Arc::new(test_record));

// 两种访问语法都会验证：
// (&record:get user :email) 会产生警告
// (user.-email) 也会产生警告：
// [Warn] Field `email` does not exist in record `Person`.
//        Available fields: [age, name]
```

**实现细节（2026-01-09）：**

文件修改：

- `src/runner/preprocess.rs`:
  - 扩展 `check_record_field_access()` 函数支持 `Method(Access)` 检查
  - 在预处理 `Proc`/`Import`/`Method` 调用时触发字段验证
  - 新增 4 个单元测试（总计 11 个测试）

工作原理：

1. 用户使用 `assert-type user <record-instance>` 标注变量类型
2. 类型信息存储在 `scope_types: HashMap<Arc<str>, Arc<Calcit>>`
3. 预处理到字段访问时（两种语法）：
   - `&record:get user :field` - 直接函数调用
   - `user.-field` - 解析为 `(.-field user)` 方法调用
4. 检查器提取 Record 类型，验证字段存在
5. 不存在则生成编译时警告

**下一步计划（阶段 4 剩余工作）：**

1. ~~**支持 `.-field` 方法访问语法的验证**~~ ✅ 已完成

   - ✅ 当前支持 `&record:get` 和 `.-field` 两种语法
   - ✅ 完整测试覆盖

2. **基于 Record 字面量的类型推断**

   - 当看到 `&%{} (&new-record :Person :name :age) ...` 时自动推断类型
   - 无需显式 `assert-type` 标注
   - 需要在 `preprocess_list_call` 中识别 `&new-record` 调用

3. **Enum 变体验证**

   - 检查 `tag-match` 中的变体是否在 `defenum` 声明中
   - 验证变体参数数量匹配
   - 需要实现 Enum 类型的存储和查询

4. **`hint-fn` 的收集和验证**
   - 当前只是解析，未实际收集返回类型信息
   - 需要在函数定义中提取 `hint-fn` 信息并存储到 `CalcitFn.return_type`
   - 需要在函数调用时验证返回类型匹配

**阶段 4 已完成部分：**

- ✅ Record 字段静态验证（`&record:get` 和 `.-field`）
- ✅ 编译时警告生成
- ✅ 完整单元测试覆盖（11 个测试）

**实现细节：**

文件修改：

- `src/runner/preprocess.rs`:
  - 添加了 `check_record_field_access()` 函数
  - 添加了 `check_field_in_record()` 辅助函数
  - 支持 `Proc(NativeRecordGet)`、`Import` 和 `Method(Access)` 三种形式
  - 在预处理 `Proc`/`Import`/`Method` 调用时触发字段验证
  - 新增 4 个单元测试验证功能

工作原理：

1. 用户使用 `assert-type user <record-instance>` 标注变量类型
2. 类型信息存储在 `scope_types: HashMap<Arc<str>, Arc<Calcit>>`
3. 预处理到 `&record:get user :field` 或 `user.-field` 时：
   - 检查 `user` 在 `scope_types` 中的类型
   - 如果是 `Calcit::Record`，提取字段列表
   - 使用 `record.index_of(field_name)` 验证字段存在
   - 不存在则生成编译时警告

**当前限制：**

- 需要显式使用 `assert-type` 标注，无法自动推断 Record 字面量类型
- 警告信息在预处理阶段生成，不影响运行时
- 不支持动态字段名（如变量或表达式计算的字段名）

**待完善：**

阶段 4 工作要点：

1. **Record 字段校验**：在 `preprocess_list_call` 中读取 `assert-type` 写入的 `Record` 元数据，给方法调用提供字段列表；
2. **实战测试**：针对 demo Cirru，引入 `assert-type x :User` 并在后续 `get x :name` 之类的调用里验证字段存在；
3. **Enum variant 支持**：为 `defenum` 生成的构造器补充 `assert-type` 示例，确保 matcher 可以根据 variant 推断类型；
4. **错误诊断**：增补一组 `runner::preprocess` 测试，模拟非法字段并验证 `LocatedWarning` 输出。

## 7. 已完成的核心改动总结（截至 2026-01-08）

**重要说明**：以下总结的是当前 **原型实现**，需要完善为 `assert-type` 和 `hint-fn` 语法。

### 7.1 数据结构变更

| 文件                   | 修改内容                                                   | 状态                                   |
| ---------------------- | ---------------------------------------------------------- | -------------------------------------- |
| `src/calcit/local.rs`  | `CalcitLocal` 已扩展 `type_info: Option<Arc<Calcit>>` 字段 | ✅ 完成，无需修改                      |
| `src/calcit/syntax.rs` | 新增 `CalcitSyntax::AssetType` 枚举项                      | ⚠️ 需要重构为 `AssertType` 和 `HintFn` |

### 7.2 语法解析（需要重构）

| 文件                | 当前实现状态                                                                                      | 需要修改为                                                                                             |
| ------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `src/data/cirru.rs` | 识别语法字符串并转换为 `CalcitSyntax` 枚举<br>单测：`parses_assert_type_list`（可能需要更新命名） | 确保正确识别 `"assert-type"` → `CalcitSyntax::AssertType`<br>识别 `"hint-fn"` → `CalcitSyntax::HintFn` |

### 7.3 预处理器核心逻辑（需要重构）

**`src/runner/preprocess.rs`** - 当前状态：

- ✅ 引入 `ScopeTypes = HashMap<Arc<str>, Arc<Calcit>>` 类型别名（无需修改）
- ✅ 所有 `preprocess_*` 函数签名新增 `scope_types: &mut ScopeTypes` 参数（无需修改）
- ✅ `preprocess_expr` 在转换 `Symbol` → `Local` 时从 `scope_types` 查询并填充 `type_info`（无需修改）
- ⚠️ **需要重构**：将 `preprocess_asset_type` 函数拆分为：
  - `preprocess_assert_type(expr, scope_types)` - 处理 `assert-type var type` 语法
  - `preprocess_hint_fn(expr, fn_info)` - 处理 `hint-fn $ return-type type` 语法
- ✅ `preprocess_defn`/`preprocess_core_let` 创建新作用域时克隆父级 `scope_types`（无需修改）
- ⚠️ 单测需要更新：`passes_asset_type_through_preprocess` → `passes_assert_type_through_preprocess`
- ⚠️ 单测需要更新：`propagates_type_info_across_scope`（更新测试代码中的语法）

**`src/builtins/syntax.rs`** - 当前状态：

- ✅ `macroexpand_all` 和相关宏处理函数调用 `preprocess_expr` 时传入 `scope_types`（无需修改）

**`src/codegen/gen_ir.rs`** - 当前状态：

- ✅ `dump_code` 生成 IR 时包含 `Local` 的 `type-info` 字段（无需修改）

### 7.4 类型传播机制（原型实现，需要适配新语法）

当前的传播路径（基于 `assert-type`）：

1. **标注点**：`assert-type x :number` 解析为 `CalcitSyntax::AssertType` 节点
2. **写入映射**：`preprocess_assert_type` 将 `x` → `:number` 写入 `scope_types`
3. **回填 Local**：后续任何对 `x` 的引用（`Symbol` → `Local` 转换时）都会从 `scope_types` 中查询并填充 `type_info`
4. **作用域传递**：`defn`/`&let` 等创建新作用域时克隆父级 `scope_types`，确保类型信息在嵌套作用域内可见
5. **函数元数据**：`hint-fn $ return-type :number` 更新当前函数定义的 `FnInfo` 结构

### 7.5 测试覆盖（需要更新语法）

| 测试用例                                                           | 验证内容                                                           | 状态                          |
| ------------------------------------------------------------------ | ------------------------------------------------------------------ | ----------------------------- |
| `data::cirru::tests::parses_assert_type_list`                      | 验证 Cirru 解析器能正确识别 `assert-type` 语法                     | ⚠️ 需要检查测试名称是否已更新 |
| `runner::preprocess::tests::passes_assert_type_through_preprocess` | 验证 `assert-type` 表达式能通过预处理器                            | ⚠️ 需要检查测试名称是否已更新 |
| `runner::preprocess::tests::propagates_type_info_across_scope`     | 验证类型信息在 `&let` 作用域内传播，后续引用同一变量时保留类型标注 | ⚠️ 需要更新测试代码语法       |

---

**重构优先级**：

1. **高优先级**：更新语法解析（`src/data/cirru.rs` 和 `src/calcit/syntax.rs`）
2. **高优先级**：重构预处理器函数（`src/runner/preprocess.rs`）
3. **中优先级**：添加函数元数据结构和 `hint-fn` 处理
4. **低优先级**：更新测试用例语法

---

阶段 4 工作要点：

---

_评估日期: 2026-01-08 · 开发状态：阶段 3 已完成，进入阶段 4_

## 8. 后续开发任务清单（优先级排序）

### 8.1 阶段 4 前置准备（估时：0.5 天）

- [ ] **任务 4.1**：梳理现有 Record 定义方式，确认 `defrecord` 语法和元数据存储位置

  - 查找 `src/builtins/` 中与 `defrecord` 相关的实现
  - 确认 Record 字段列表如何在程序运行时被访问
  - 编写测试 demo：定义一个简单 Record 并验证字段提取

- [ ] **任务 4.2**：设计类型查询接口
  - 在 `program.rs` 或新建模块中添加 `lookup_record_fields(ns: &str, record_name: &str) -> Option<Vec<String>>` 接口
  - 支持从 `assert-type` 的类型标注（如 `:User`）反查字段列表

### 8.2 Record 字段静态校验（估时：2 天）

- [ ] **任务 4.3**：实现方法调用的 Record 字段验证

  - 在 `preprocess_list_call` 中识别 `get`/`.-field` 等方法调用
  - 当第一个参数是带有 Record `type_info` 的 `Local` 时，检查字段名是否在 Record 定义中
  - 不匹配时生成 `LocatedWarning` 并记录到 `check_warnings`

- [ ] **任务 4.4**：编写 Record 字段校验单测

  - 测试用例 1：合法字段访问（如 `get user :name` 当 `user` 标注为 `:User` 且 `:name` 存在）
  - 测试用例 2：非法字段访问（如 `get user :age` 当 `:age` 不存在于 Record 定义）
  - 测试用例 3：未标注类型的变量不触发校验

- [ ] **任务 4.5**：在实际 Cirru demo 中验证
  - 在 `calcit/` 或 `demos/` 下创建一个包含 Record 定义和使用的 `.cirru` 文件
  - 故意写错字段名，运行 `cr --check-only` 验证是否正确报告警告

### 8.3 Enum variant 支持（可选，估时：1.5 天）

- [ ] **任务 4.6**：调研 `defenum` 实现现状

  - 查看是否已有 `defenum` 或类似的枚举定义语法
  - 如果没有，评估是否需要在此阶段实现，或先用 Record 代替

- [ ] **任务 4.7**：为 Enum 实现 variant 校验
  - 在 pattern matching 或 `case` 表达式中检查 variant 名称合法性
  - 生成对应的 `LocatedWarning`

### 8.4 内置函数类型提示（估时：1.5 天）

- [ ] **任务 4.8**：设计内置函数签名描述格式

  - 在 `src/builtins.rs` 或 `CalcitProc` 中扩展元数据结构
  - 定义简单的类型签名格式（如 `"(fn (a b) c)"` 或使用 Calcit 值表示）

- [ ] **任务 4.9**：为常用内置函数添加签名

  - 优先覆盖：`+`、`-`、`*`、`/`、`str`、`get`、`assoc`、`dissoc`、`map`、`filter` 等高频函数
  - 编写单测验证签名可被查询

- [ ] **任务 4.10**：在预处理器中使用内置函数签名
  - 当调用 `Proc` 时，根据签名进行参数类型检查
  - 生成类型不匹配的 `LocatedWarning`（初期可以只做参数数量校验）

### 8.5 运行时类型查看（估时：1 天）

- [ ] **任务 4.11**：实现 `&inspect-type` 原语

  - 在 `src/builtins/` 中添加新的 proc，接受一个 `Local` 并返回其 `type_info`
  - 支持在 REPL 或测试代码中动态查看类型标注

- [ ] **任务 4.12**：增强错误信息显示
  - 当类型相关的 `LocatedWarning` 被打印时，包含变量的 `type_info` 内容
  - 在 `LocatedWarning::print_list` 中优化格式

### 8.6 文档与示例（估时：1 天）

- [ ] **任务 4.13**：编写用户文档

  - 在 `docs/` 下创建 `type-annotations.md`，说明 `assert-type` 和 `hint-fn` 语法
  - 提供 Record 字段校验的示例代码
  - 说明 `unknown` 类型的语义和向后兼容策略

- [ ] **任务 4.14**：更新 README 和 changelog
  - 在 `README.md` 中添加类型标记功能的简介
  - 记录到版本 changelog（如果计划发布）

### 8.7 集成测试与性能优化（估时：1 天）

- [ ] **任务 4.15**：运行完整测试套件

  - 确保所有现有测试仍然通过
  - 检查预处理器性能，尤其是大型项目中的类型映射查找开销

- [ ] **任务 4.16**：性能优化（如需要）
  - 考虑使用 `Arc` 或引用计数减少 `ScopeTypes` 克隆开销
  - 对热路径（如 `preprocess_expr` 中的 Local 转换）进行 profiling

---

### 📋 任务执行顺序建议

**第一周**（优先完成 Record 校验核心功能）：

1. 任务 4.1, 4.2（前置准备）
2. 任务 4.3, 4.4（Record 字段校验核心）
3. 任务 4.5（实战验证）

**第二周**（扩展功能与完善）： 4. 任务 4.11, 4.12（运行时支持） 5. 任务 4.8, 4.9（内置函数签名，部分） 6. 任务 4.13, 4.14（文档）

**第三周**（收尾与优化）： 7. 任务 4.10（内置函数类型检查） 8. 任务 4.15, 4.16（集成测试与性能） 9. 可选：任务 4.6, 4.7（Enum 支持）

---

_评估日期: 2026-01-08 · 开发状态：阶段 3 已完成，进入阶段 4_

## 6. `defenum` 设计草案

为了在 tuple 上实现更强的代数数据类型能力，可以引入轻量的 `defenum` 语法，为现有的 `tag-match` 提供结构化的类型声明。

### 6.1 现有基础：`tag-match`

Calcit 已经具备基于 tagged tuple 的模式匹配能力：

```cirru
; 现有用法
tag-match (:: :ok 1)
  (:ok v) (&+ v 10)
  (:err e) (eprintln e)

tag-match (:: :some |hello)
  (:some x) (str |got: x)
  (:none) |nothing
```

**现状分析**：

- ✅ 已有 `::` 语法创建 tagged tuple
- ✅ 已有 `tag-match` 宏进行 pattern matching
- ✅ 支持参数数量检查（`tag-match` 会验证 tuple 长度）
- ❌ 缺少类型声明：无法事先定义合法的 variant 集合
- ❌ 缺少静态校验：拼写错误的 tag 名称不会被发现

### 6.2 用 `defrecord` 表达枚举（变体用属性数组）

为减少新语法引入的复杂度，使用已有的 `defrecord` 来表示枚举，每个变体作为一个 `Record` 类型，参数改为属性数组进行约束：

```cirru
; 一个定义包含所有变体：每个 field 对应一个 enum 的 tag
defrecord! Result
  :ok $ [] V1
  :err $ [] :string

defrecord! Option
  :some $ [] V1
  :none $ []

defrecord! Event
  :click $ [] :number :number
  :keypress $ [] :string :bool
  :message $ [] :string VUser :number

; 使用时用 assert-type 标注，并通过属性数组校验字段
defn handle-result (result)
  assert-type result :Result
  tag-match result
    (:ok v) (println |Success: v)
    (:err e) (println |Error: e)
```

说明：上述 `Result` 定义中，`V1` 表示某个具体的 Record 类型（例如用户自定义的 `Record`），而 `:string` 则表示内置的字符串类型标记。

为了承载这些 enum 元数据，tuple 构造函数新增 `%%::` 形式：

```cirru
%%:: Action Result :ok payload
```

它会把 `Action` 作为 class/trait，同时把 `Result` 作为 sum type。对应实现中，`CalcitTuple` 增加了 `sum_type` 字段，以便方法分派(`class`)和 `tag-match` 校验(`sum_type`)可以同时使用这些信息。

**要点**：

- 变体是现有 `Record`，字段由属性数组限定；
- 通过 `assert-type` 为变量标注为具体变体或联合（如 `|or` 简写联合）；
- 预处理时利用 `Record` 元数据进行字段存在性校验，避免新增枚举存储结构；
- 与现有 `record-match`/`&record:get` 检查机制复用，减少实现成本。

## 9. `hint-fn` 返回值标记落地计划（2026-01-09）

**目标**：让 `hint-fn` 中的 `return-type` 标记真正落地到 `CalcitFn.return_type`，并保持对既有 `hint-fn async` 提示的兼容，随后逐步在 `calcit-core` 中补充这些元信息。

### 9.1 实现路径

1. **入口选择**：在 `builtins/syntax::defn` 中扫描函数体（`expr.skip(2)`）里所有 `hint-fn` 语句，统一解析出 `return-type` 提示；`hint-fn async` 继续保留在函数体里，以便现有 JS codegen 检测异步标记。
2. **数据落地**：一旦捕获到类型表达式，就以 `Arc<Calcit>` 形式写入 `CalcitFn.return_type`。同一个函数出现多次 `return-type` 时以后出现者覆盖之前的值，方便局部覆盖。
3. **语法支持**：统一为 `hint-fn $ return-type :number` 的写法（必须通过 `$` 包裹参数），内部解析器仅解析该嵌套列表形式，`hint-fn` 的其它用法（如 `hint-fn async`）保持不变。
4. **单元测试**：在 `builtins/syntax.rs` 增加针对解析器与 `defn` 的测试，覆盖：

- 能识别 `$` 形式的 `return-type`。
- 忽略只有 `hint-fn async` 的场景。

5. **验证流程**：`cargo test builtins::syntax`（或新增测试名）+ `cargo run --bin cr -- -1 calcit/test-types.cirru ir`，确保 IR 中 `arg-types`/`return-type` 字段包含新的类型信息。

### 9.2 后续计划（实现完成后立即跟进）

1. 在 `calcit/test-types.cirru` 与 `calcit-core.cirru` 中挑选一批核心函数，补写 `hint-fn $ return-type ...` 以展示元数据；
2. 利用 `rg -n ":type-info" js-out/program-ir.cirru` 快速确认输出里新增的 `return-type`；
3. 运行 `yarn check-all` 确认核心库在补充标注后仍保持绿色；
4. 如需暴露返回值信息给其它工具，再按需扩展 IR 消费端或 CLI 输出。

> 备注：`assert-type` → `CalcitFn.arg_types` 的聚合将在后续步骤里处理，本次先把 `hint-fn` 链路打通，便于后面落地更大范围的类型可视化。
