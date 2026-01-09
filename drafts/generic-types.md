# Calcit 局部变量类型标记方案评估

本文档评估在 Calcit 中为 `Local` 变量补充类型信息的技术方案及工作量。涉及任务进度与交付计划请参阅 `drafts/generic-types-plan.md`。

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

#### 2.0.7 泛型类型支持

为支持泛型编程，Calcit 使用 **quoted symbol** 表示类型变量，利用现有的 Record 和 Symbol 语法实现零额外语法成本的泛型系统。

##### 基本语法

**类型变量表示**：

- 使用单引号 `'` 加大写字母开头的 symbol 表示类型变量：`'T`, `'U`, `'K`, `'V`
- 类型变量遵循词法作用域：在同一函数内，相同 symbol 表示同一类型
- 类型变量不需要显式声明，首次使用即生效

**简单泛型函数**：

```cirru
; 恒等函数：接受任意类型 T，返回相同类型 T
defn identity (x)
  assert-type x 'T
  hint-fn $ return-type 'T
  x

; 交换 pair：接受 (A, B)，返回 (B, A)
defn swap-pair (pair)
  assert-type pair $ :tuple 'A 'B
  hint-fn $ return-type $ :tuple 'B 'A
  :: (nth pair 1) (nth pair 0)

; 列表头部：接受 List<T>，返回 T
defn head (xs)
  assert-type xs $ :list 'T
  hint-fn $ return-type 'T
  first xs
```

**泛型函数类型**：

```cirru
; map 函数：(T -> U) -> List<T> -> List<U>
defn my-map (f xs)
  assert-type f $ :fn ('T) 'U
  assert-type xs $ :list 'T
  hint-fn $ return-type $ :list 'U
  map f xs

; filter 函数：(T -> Bool) -> List<T> -> List<T>
defn my-filter (pred xs)
  assert-type pred $ :fn ('T) :bool
  assert-type xs $ :list 'T
  hint-fn $ return-type $ :list 'T
  filter pred xs

; fold 函数：(B -> A -> B) -> B -> List<A> -> B
defn my-fold (f init xs)
  assert-type f $ :fn ('B 'A) 'B
  assert-type init 'B
  assert-type xs $ :list 'A
  hint-fn $ return-type 'B
  fold f init xs
```

**泛型 Record 字段**：

```cirru
; 定义泛型容器 Record（通过注释说明，实际字段存储具体类型）
defrecord! Box
  :value 'T  ; 注释：表示 value 字段可以是任意类型

; 使用时指定具体类型
defn make-int-box (n)
  assert-type n :number
  hint-fn $ return-type $ :Box :number  ; Box<number>
  %{} Box :value n

defn unbox (box)
  assert-type box $ :Box 'T
  hint-fn $ return-type 'T
  &record:get box :value
```

##### 类型约束（高级）

对于需要类型约束的场景，可以使用 Record 包装：

```cirru
; 方案 A: 使用专用 Record 表达约束
defrecord! GenericType
  :var 'symbol       ; 类型变量名
  :constraints []    ; 类型约束列表

defn sort-by (cmp xs)
  ; cmp: (T, T) -> Bool where T: Ord
  assert-type cmp $ :fn
    , $ %{} GenericType :var 'T :constraints $ [] :Ord
    , $ %{} GenericType :var 'T :constraints $ [] :Ord
    , :bool
  assert-type xs $ :list 'T
  hint-fn $ return-type $ :list 'T
  ; ... 排序实现

; 方案 B: 简化语法糖（可选）
defn max-value (a b)
  assert-type a $ 'T :where :Ord
  assert-type b 'T
  hint-fn $ return-type 'T
  if (&> a b) a b
```

##### 高阶类型（高级）

Record 可表达高阶类型变量（Kind: _ -> _）：

```cirru
; fmap: (A -> B) -> F<A> -> F<B>
defn fmap (f container)
  assert-type f $ :fn ('A) 'B
  assert-type container $ %{} HigherType
    :kind 'F           ; F 是类型构造器
    :param 'A          ; 应用到类型参数 A
  hint-fn $ return-type $ %{} HigherType
    :kind 'F
    :param 'B
  ; ... 实现依赖于容器类型
```

##### 类型统一与检查

**预处理阶段**：

- 维护 `GenericBindings: HashMap<Symbol, Calcit>` 追踪类型变量的具体绑定
- 首次遇到类型变量时记录为泛型
- 后续遇到相同变量时进行类型统一检查
- 不匹配时生成 `LocatedWarning`

**示例 - 类型不一致检测**：

```cirru
defn bad-usage (x y)
  assert-type x 'T
  assert-type y 'T
  ; 如果调用 (bad-usage 1 "hello")
  ; 预处理器应警告：T 被绑定为 :number，但 y 传入了 :string
```

##### 实现阶段规划

**阶段 1：基础泛型变量**

- 识别 quoted symbol 作为类型变量（首字母大写）
- 在 `TypeContext` 中维护泛型绑定
- 在同一作用域内检查类型一致性

**阶段 2：泛型类型统一**

- 实现简单的类型统一算法
- 支持嵌套泛型类型（如 `:list 'T`）
- 提供清晰的错误信息

**阶段 3：约束与高阶类型**

- 支持类型约束（trait bounds）
- 支持高阶类型变量
- 完善泛型推断能力

##### 优势与限制

**✅ 优势**：

1. **零语法成本**：复用现有 Symbol 和 Record，无需引入 `<T>` 或 `forall` 等特殊语法
2. **渐进式增强**：完全向后兼容，可选择性使用泛型标注
3. **表达力强**：Record 可表达复杂约束和高阶类型
4. **一致性好**：与现有类型系统设计理念高度一致

**⚠️ 限制**：

1. **作用域限制**：类型变量目前仅支持函数级作用域
2. **推断有限**：不支持完整的 Hindley-Milner 类型推断
3. **约束系统待完善**：trait/typeclass 约束系统需要进一步设计
4. **错误信息**：泛型统一失败的诊断信息需要优化

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

6. **泛型类型处理**（新增）：

   预处理器需要增强类型表达式解析，支持泛型变量的识别与统一：

   **实现要点**：

   - 扩展 `TypeContext` 增加 `generic_bindings: HashMap<Symbol, Calcit>` 追踪泛型绑定
   - 识别 quoted symbol（首字母大写）作为泛型变量
   - 首次遇到泛型变量时标记为 `generic:T`，后续遇到时进行类型统一检查
   - 支持嵌套泛型类型的递归解析（如 `:list 'T`）
   - 类型不匹配时生成 `LocatedWarning`

   **Calcit 代码示例**：

   ```cirru
   ; 示例 1: 基本泛型函数
   defn identity (x)
     type-variable 'T
     assert-type x 'T
     hint-fn $ return-type 'T
     , x

   ; 调用示例
   identity 42        ; T := :number
   identity "|hello"   ; T := :string

   ; 示例 2: 类型一致性检查
   defn must-same-type (a b)
     type-variable 'T
     assert-type a 'T
     assert-type b 'T   ; 必须与 a 类型相同
     hint-fn $ return-type 'T
     if (&> a b) a b

   must-same-type 1 2     ; ✓ T := :number
   must-same-type 1 "|hi"  ; ✗ 警告：T 已绑定为 :number，但传入 :string

   ; 示例 3: 泛型列表操作
   defn first-and-second (xs)
     type-variable 'T
     assert-type xs $ :list 'T
     hint-fn $ return-type $ :tuple 'T 'T
     :: (first xs) (nth xs 1)

   first-and-second $ [] 1 2 3        ; ✓ T := :number
   first-and-second $ [] |a |b      ; ✓ T := :string

   ; 示例 4: 高阶函数泛型
   defn my-map (f xs)
     type-variable 'T
     type-variable 'U
     assert-type f $ :fn ('T) 'U
     assert-type xs $ :list 'T
     hint-fn $ return-type $ :list 'U
     map f xs

   my-map str $ [] 1 2 3              ; T := :number, U := :string
   my-map &str:blank? $ [] |a |     ; T := :string, U := :bool

   ; 示例 5: 多态容器
   defn wrap-value (v)
     type-variable 'T
     assert-type v 'T
     hint-fn $ return-type $ :Box 'T
     %{} Box (:value v)

   defn unwrap-value (box)
     type-variable 'T
     assert-type box $ :Box 'T
     hint-fn $ return-type 'T
     &record:get box :value

   let box $ wrap-value 100           ; Box<number>
     unwrap-value box                 ; -> 100 : number

   ; 示例 6: 带约束的泛型（未来扩展）
   defn sort-list (xs)
     type-variable 'T
     assert-type xs $ :list
       %{} GenericType
         :var 'T
         :constraints $ [] :Ord
     hint-fn $ return-type $ :list 'T
     sort xs

   ; 示例 7: 复杂泛型函数组合
   defn pipe-two (f g x)
     type-variable 'A
     type-variable 'B
     assert-type f $ :fn ('A) 'B
     assert-type g $ :fn ('B) 'C
     assert-type x 'A
     hint-fn $ return-type 'C
     g $ f x

   pipe-two parse-int &str:reverse "|123"  ; A := :string, B := :number, C := :string
   ```

   **类型统一示例**：

   ```cirru
   ; 正确：类型一致
   defn example-ok (a b c)
     assert-type a 'T
     assert-type b 'T
     assert-type c 'T
     ; 如果调用 (example-ok 1 2 3)，T 统一为 :number

   ; 错误：类型冲突
   defn example-bad (a b)
     assert-type a 'T
     ; ... 一些代码
     assert-type b 'T
     ; 如果调用 (example-bad 1 "|hello")
     ; 预处理器警告：Type mismatch for 'T': expected :number, got :string
   ```

   **IR 输出示例**：

   ```cirru
   ; 编译后的 program-ir.cirru 中可见
   :local $ {} (:idx 0) (:sym |x)
     :type-info $ :tag "|generic:T"

   :fn $ {}
     :return-type $ :tag "|generic:T"
     :arg-types $ [] (:tag "|generic:T")
   ```

### 2.3 内置函数与类型提示

- **Procs 注册**：在 `src/builtins.rs` 中，需要一种方式为 `CalcitProc` 关联签名信息。
- **兼容性**：老代码保持 `type_info` 为 `None` (即 `unknown`)，不进行强校验。

### 2.4 运行时支持

- 增加 `&inspect-type` 原语，允许在运行时通过 `CalcitLocal` 获取其标记的类型名。

## 3. `defenum` 设计草案

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

## 4. `hint-fn` 返回值标记落地纪要（2026-01-09）

本轮迭代完成了 `hint-fn $ return-type ...` → `CalcitFn.return_type` 的闭环，实现细节与验证路径记录如下。

### 9.1 实现成果

- `builtins/syntax::defn` 在展开函数体时会扫描全部 `hint-fn` 语句，遇到 `$ return-type ...` 结构即解析出类型表达式，后一条提示可覆盖前一条，最终写入 `CalcitFn.return_type`。
- `hint-fn async` 的既有语义保持不变，会被原样保留在函数体中供 JS codegen 检测异步函数。
- `calcit-core.cirru` 与 `calcit/test-types.cirru` 已补充一批 `hint-fn` 标注来喂数，`cargo run --bin cr -- -1 calcit/test-types.cirru ir` 后可在 `program-ir.cirru` 中看到 `return-type` 字段；同样可以通过 `rg return-type js-out/program-ir.cirru` 快速确认导出的 IR。
- 新增的 `builtins::syntax`/`program::ir` 单元测试覆盖了解析与序列化流程，确保 `return-type` 元信息在 AST→IR 全链路上都有快照保护。

### 9.2 后续跟进

1. 扩大 `hint-fn` 标注覆盖面：把核心库、常用模块以及示例项目中的函数逐步加上 `return-type`，累计形成可靠的数据基础。
2. 将 `return-type` 对外可视化：在 `cr query def`/`cr docs` 或 LSP 返回值里显示该字段，方便 IDE/CLI 读取。
3. 设计最小的调用点静态检查，基于 `return-type` 给出不匹配的早期提示（例如数值上下文误用了 `str`）。
4. 与 `CalcitFn.arg_types` 聚合，将完整的函数签名输出到 IR，便于后续 tooling（类型浏览器、文档生成器）消费。

> 备注：`assert-type` → `CalcitFn.arg_types` 的聚合仍在规划中，本纪要聚焦于 `return-type` 链路；相关构建和 `yarn check-all` 均已通过，确保引入的元信息不会影响现有运行路径。
