# RFC: 函数参数字面量自动改写 — Map-to-Record & Tuple-to-Enum

状态：Implemented
日期：2026-04-13

---

## 1. 概要

在预处理阶段，当函数参数的 schema 标注引用了具体类型（struct 或 enum）时，允许调用方使用更简洁的字面量语法（hashmap `{}` 或 untyped tuple `::`），由预处理器自动改写为对应的类型化构造表达式（record `%{}` 或 enum tuple `%::`），以便后续类型检查正常工作。

## 2. 动机

### 问题

Calcit 区分"无类型字面量"和"有类型构造"：

- `{} (:x 1) (:y 2)` 创建普通 hashmap，而 `%{} Point (:x 1) (:y 2)` 创建 record。
- `:: :ok` 创建普通 tuple（无 class 引用），而 `%:: Result0 :ok` 创建 enum tuple（有 class）。

当函数签名明确标注参数类型时，调用者仍需手写完整的类型化构造：

```cirru
;; 参数标注为 Point record
defn sum-point (p) ...
  :: :fn $ {} (:return :number)
    :args $ [] 'app.main/Point

;; 调用者必须写完整形式
sum-point $ %{} Point (:x 10) (:y 20)
```

这在大量调用点重复书写时既冗余又容易出错（特别是忘记加 `%` 前缀）。

### 解法

预处理器根据函数参数的 schema 类型标注，自动将简写形式改写为完整形式：

| 简写 | 改写为 | 触发条件 |
|---|---|---|
| `{} (:x 1) (:y 2)` | `%{} Point (:x 1) (:y 2)` | 参数类型为 struct/record |
| `:: :ok` | `%:: Result0 :ok` | 参数类型为 enum |

改写后的 AST 能正常参与：
- 运行时类型验证（field 校验、variant 校验）
- 预处理阶段类型检查（`check_user_fn_arg_types`）
- JS codegen（通过 Import 引用而非内联 Struct/Enum 值）

## 3. 设计

### 3.1 Map-to-Record 改写

**触发条件：**

1. 函数 schema 的 `:args` 中某位置引用了 struct 类型（`TypeRef`、`Struct` 或 `Record`）
2. 调用处该位置是 hashmap 字面量（以 `NativeMap` proc 开头的 list）
3. hashmap 所有 key 都是 tag，且都是目标 struct 的合法字段

**改写规则：**

- `[NativeMap, :k1, v1, :k2, v2]` → `[NativeRecord, struct_ref, :field1, v1_or_nil, :field2, v2_or_nil, ...]`
- struct 中未提供的字段自动填充 `nil`
- 字段顺序按 struct 定义排列（非 map 出现顺序）
- `struct_ref` 优先使用 `Calcit::Import`（带 ns/def 路径），以兼容 JS codegen

**跳过条件（不报错，保持原样）：**

- map 中含有非 tag 的 key
- map 中含有 struct 不存在的字段名
- 无法从 schema 解析出 struct 定义

**示例：**

```cirru
defstruct Point (:x :number) (:y :number)

defn sum-point (p)
  :: :fn $ {} (:return :number)
    :args $ [] 'app.main/Point
  &+ (:x p) (:y p)

;; 简写
sum-point $ {} (:x 10) (:y 20)
;; 预处理改写为
sum-point $ %{} Point (:x 10) (:y 20)
```

### 3.2 Tuple-to-Enum 改写

**触发条件：**

1. 函数 schema 的 `:args` 中某位置引用了 enum 类型（`TypeRef`、`Enum` 或 `Tuple`）
2. 调用处该位置是 untyped tuple 字面量（以 `NativeTuple` proc 开头的 list）

**改写规则：**

- `[NativeTuple, :tag, payload...]` → `[NativeEnumTupleNew, enum_ref, :tag, payload...]`
- `enum_ref` 优先使用 `Calcit::Import`（带 ns/def 路径），以兼容 JS codegen
- 不验证 tag 或 payload（由后续 `check_enum_tuple_construction` 完成）

**跳过条件（不报错，保持原样）：**

- tuple 字面量没有 tag（空 `::` 表达式）
- 无法从 schema 解析出 enum 定义

**示例：**

```cirru
defenum Result0 (:err :string) (:ok)

defn takes-result (r)
  :: :fn $ {} (:return :dynamic)
    :args $ [] 'app.main/Result0
  tag-match r ((:ok) :ok) ((:err msg) msg) $ _ :unknown

;; 简写
takes-result $ :: :ok
;; 预处理改写为
takes-result $ %:: Result0 :ok

;; 带 payload
takes-result $ :: :err |error-msg
;; 预处理改写为
takes-result $ %:: Result0 :err |error-msg
```

## 4. 实现

### 4.1 类型解析

在 `CalcitTypeAnnotation` 上新增方法：

| 方法 | 用途 |
|---|---|
| `resolve_to_struct_with_ref()` | 解析 struct + 可选 (ns, def) 路径（已有） |
| `resolve_to_enum_with_ref()` | 解析 enum + 可选 (ns, def) 路径（新增） |

两者都处理 `Struct/Record`↔`Enum/Tuple` 直接值、`TypeRef("ns/def")` 程序查找、以及 `Optional(inner)` 解包。

底层依赖：

| 函数 | 用途 |
|---|---|
| `resolve_struct_from_program(ns, def)` | 从 program registry 查找 struct 定义（已有） |
| `resolve_enum_from_program(ns, def)` | 从 program registry 查找 enum 定义（新增） |

### 4.2 改写函数

| 函数 | 用途 |
|---|---|
| `try_rewrite_map_args_to_records()` | 遍历参数列表，对 map 字面量尝试改写（已有） |
| `try_rewrite_single_map_to_record()` | 单个参数的 map→record 改写（已有） |
| `try_rewrite_tuple_args_to_enum_tuples()` | 遍历参数列表，对 tuple 字面量尝试改写（新增） |
| `try_rewrite_single_tuple_to_enum_tuple()` | 单个参数的 tuple→enum-tuple 改写（新增） |

### 4.3 集成点

在 `preprocess_list_call()` 的 `Fn` 分支中，按顺序调用：

1. `try_rewrite_map_args_to_records()` — map → record
2. `try_rewrite_tuple_args_to_enum_tuples()` — tuple → enum tuple
3. `check_core_fn_arg_types()` — 内建函数参数类型检查
4. `check_user_fn_arg_types()` — 用户函数参数类型检查

两次改写串联：第一次的输出作为第二次的输入。

### 4.4 JS Codegen 兼容

改写时若从 `TypeRef` 解析出 (ns, def) 路径，会构造 `Calcit::Import` 而非内联的 `Calcit::Struct`/`Calcit::Enum`。这是因为 JS codegen 不支持直接 emit `Struct`/`Enum` 字面量——它需要一个变量引用。

Import 策略：
- 同 namespace → `ImportInfo::SameFile`
- 跨 namespace → `ImportInfo::NsReferDef`

### 修改的文件

| 文件 | 变更 |
|---|---|
| `src/calcit/type_annotation.rs` | `resolve_to_enum_with_ref()`, `resolve_enum_from_program()` |
| `src/runner/preprocess.rs` | `try_rewrite_tuple_args_to_enum_tuples()`, `try_rewrite_single_tuple_to_enum_tuple()`，集成到 Fn 分支 |
| `docs/features/enums.md` | 新增 "Automatic Tuple-to-Enum Rewrite" 章节 |
| `docs/features/records.md` | "Automatic Map-to-Record Rewrite" 章节（已有） |
| `calcit/test-enum.cirru` | 新增 `test-tuple-to-enum` 测试 |

## 5. 测试覆盖

### Map-to-Record（已有）

- `calcit/test-record.cirru` 中 `test-map-to-record`
  - `sum-point $ {} (:x 10) (:y 20)` → 验证返回值正确
  - `check-point-type $ {} (:x 10) (:y 20)` → 验证改写后值为 record 类型

### Tuple-to-Enum（新增）

- `calcit/test-enum.cirru` 中 `test-tuple-to-enum`
  - `takes-result $ :: :ok` → 验证 tag-match 匹配
  - `takes-result $ :: :err |error-msg` → 验证 payload 传递
  - `check-result-type $ :: :ok` → 验证改写后值有 enum origin

## 6. 局限性

- **仅作用于直接字面量**：传递变量（即使值是 map/tuple）不会触发改写，因为预处理阶段无法确定变量的运行时值。
- **不验证内容**：map-to-record 验证 key 是否为合法字段，tuple-to-enum 不验证 tag 和 payload（由后续的 `check_enum_tuple_construction` 完成）。
- **无法逆向**：改写后的 AST 无法区分是用户手写的 `%{}` / `%::` 还是改写生成的。调试时看到的都是改写后的形式。
- **JS codegen 依赖**：如果类型注解是直接的 `Struct(def)` / `Enum(def)` 而非 `TypeRef("ns/def")`，改写会内联结构体/枚举值，在 JS codegen 中可能 panic（目前通过优先使用 TypeRef 规避）。
