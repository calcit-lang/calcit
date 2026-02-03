# 泛型 Struct/Fn/Proc 方案说明

## 目标语法

### Struct 定义

支持：

- `defstruct Job ('T 'S) (:title 'T) (:status 'S)`

说明：

- `('T 'S)` 为类型变量列表。
- `('T)` 中的 `'T` 视为“类型变量标记”，等价于 `(quote T)` 执行一次后得到的 `symbol`。
- 若没有泛型参数，允许写 `defstruct Job (:title :string)`，但在**类型注释**中仍统一使用 `(:: Job)`。

### Struct 类型引用

统一形式：

- `(:: Job 'T 'S)`
- 无泛型参数也写 `(:: Job)`

### 函数定义 (fn)

通过 `hint-fn` 声明泛型变量：

- `hint-fn (:: :generics 'T 'S)`
- 此声明仅用于建立函数体内类型变量的自由绑定。
- `hint-fn` 的其它条目（如 `return-type`）保持兼容。

### Proc 类型签名

扩展 `:: :fn` 的语法：

- `:: :fn ('T) ('T) 'T` 代表函数签名，且**第一组括号包含泛型变量声明**。
- 若第一组括号中**不出现** `'T` 形式，则认为“泛型省略”。

## 数据结构调整

### `CalcitStruct`

新增字段：

- `generics: Arc<Vec<Arc<str>>>`

用于保存结构体定义时声明的泛型变量。

### `CalcitFnTypeAnnotation`

新增字段：

- `generics: Arc<Vec<Arc<str>>>`

保存函数类型签名中声明的泛型变量。

### `CalcitTypeAnnotation` 新增变体

- `TypeVar(Arc<str>)`：对应 `'T`
- `AppliedStruct { base: Arc<CalcitStruct>, args: Arc<Vec<Arc<CalcitTypeAnnotation>>> }`
  - 表示 `(:: Job 'T 'S)`
  - 当 `args` 为空时代表 `(:: Job)`

> 若希望最小改动，也可在 `Struct` 变体中内嵌一个 `args` 字段，但将影响大量匹配逻辑。

## 解析行为调整

### `defstruct` 解析

支持 `defstruct Name (generics...) (field type)...`：

- 解析时将 `('T 'S)` 转换为 `Vec<Arc<str>>` 存入 `CalcitStruct.generics`。
- `defstruct` 宏（`src/cirru/calcit-core.cirru`）和 `&struct::new`（Rust）都需要支持额外参数。

### `::` 类型解析

在 `parse_type_annotation_form` 中：

- 识别 `(:: Job 'T 'S)` 为 `AppliedStruct`。
- 当 `::` 后的符号解析为 struct 定义时，读取 `CalcitStruct.generics`，并校验参数个数。

### `'T` 识别

- 当类型注释遇到 `Calcit::List`/`Calcit::Tuple` 中的 `Symbol` 或 `Quote` 表达式：
  - 形如 `'T` 识别为 `TypeVar("T")`。

### `hint-fn` 泛型声明

在 `preprocess` 中：

- 扫描 `hint-fn` 的 top-level items。
- 遇到 `(:: :generics 'T 'S)` 时，将泛型变量存入当前函数签名上下文。

### Proc `:fn` 签名扩展

在 `parse_type_annotation_form` 中：

- 对 `(:: :fn ...)` 的解析扩展：
  - 若第一个列表中的元素包含 `'T`，视为泛型声明列表。
  - 其后列表视为参数列表。
  - return type 仍为最后一项。

## 类型匹配逻辑 (核心)

### 目标行为

当遇到 `TypeVar("T")`：

- 若尚未绑定，记录 `T -> 实际类型`。
- 若已绑定，则要求当前类型与已绑定类型匹配。

### 匹配上下文

- 将 `matches_annotation` 扩展为接收 `&mut TypeBindings`。
- `TypeBindings` 可使用 `HashMap<Arc<str>, Arc<CalcitTypeAnnotation>>`。

### Struct 泛型匹配

- 若 `AppliedStruct` 与 `Struct` 对应：
  - `AppliedStruct.args` 和 `CalcitStruct.generics` 建立绑定。
  - 字段类型中出现 `TypeVar` 时使用绑定。

### `:fn` 泛型匹配

- 以 `CalcitFnTypeAnnotation.generics` 声明为绑定范围。
- 对 `arg_types` 和 `return_type` 进行匹配时应用绑定。

## 影响文件清单

- `src/calcit/calcit_struct.rs`
- `src/calcit/type_annotation.rs`
- `src/calcit/struct.rs`（若仍保留）
- `src/builtins/records.rs` (`&struct::new`)
- `src/runner/preprocess.rs` (hint-fn 解析)
- `src/cirru/calcit-core.cirru` (defstruct 宏)
- 测试文件：
  - `calcit/test-types.cirru`
  - `calcit/test-types-inference.cirru`

## 兼容性与迁移

- 旧语法 `defstruct Job (:title :string)` 保留。
- **类型注释统一写法**：旧写法 `Job` 改为 `(:: Job)`。
- 若 `AppliedStruct.args` 缺少参数，默认视为 `Dynamic` 并给出 warn。
- 若多于定义参数，给出 warn 并忽略多余部分。

## 测试建议

- Struct 泛型：
  - `defstruct Job ('T 'S) ...` + `(:: Job :string :number)`
- 嵌套泛型：
  - 字段类型为 `(:: List 'T)`
- Proc 签名：
  - `:: :fn ('T) ('T) 'T`
- Fn 注释：
  - `hint-fn (:: :generics 'T)` + `return-type` 组合
