# RFC: Type Slot 机制 — 库-应用间的类型注入

状态：Draft
日期：2025-07-13

---

## 1. 概要

实现 `deftype-slot` / `bind-type` proc 对，允许库代码声明类型占位符（slot），应用代码在启动时绑定具体类型（enum/struct/record）。预处理阶段自动解析 slot 引用，使跨包边界的类型检查成为可能。

## 2. 动机

### 问题

Calcit 的静态类型分析在单包内工作良好：`defenum` 定义的变体名、载荷数量和类型都能在预处理阶段被检查。但当**库**（如 Respo）定义回调签名时，它无法知道**应用**层会使用哪个 enum 作为 dispatch 操作类型。

以 Respo 的 `EventHandler` 为例：

```cirru
;; respo.schema — 库代码
;; dispatch 回调的签名原先只能标注为 :tuple（即 Dynamic）
:: :fn $ {} (:return :unit)
  :args $ [] '*dispatch-op
```

应用层定义了自己的 Op enum：

```cirru
;; app.schema
defenum Op (:add :string) (:remove :tag) (:toggle :map) (:clear) ...
```

没有 type-slot 机制时，`d! (: add ...)` 这类调用无法被类型检查，因为预处理器不知道 dispatch 参数应该是 `Op` 类型。

### 为什么不用泛型

Calcit 目前没有完整的泛型（type parameter）系统。引入泛型会大幅增加语言复杂性，而 type-slot 解决的是一个更窄的问题：**跨编译单元的单一类型注入**。它更像 dependency injection 而非 parametric polymorphism。

## 3. 设计

### 核心概念

| 概念 | 说明 |
|---|---|
| **Type Slot** | 一个命名占位符，声明时值为 `None`，绑定后值为具体类型注解 |
| `deftype-slot :name` | 在库代码中声明 slot（通常放在 schema 命名空间） |
| `bind-type :name ConcreteType` | 在应用入口绑定具体类型（通常放在 `main!` 函数体） |
| `*name` | 在 schema 类型标注中引用 slot（解析为 `TypeSlot(name)`） |

### 生命周期

```
声明 (deftype-slot)  →  绑定 (bind-type)  →  解析 (*name 引用)  →  类型检查
       ↑ 库代码               ↑ 应用入口           ↑ 预处理阶段          ↑ 预处理阶段
```

1. **声明**：`deftype-slot :dispatch-op` 在 `TYPE_SLOTS` 注册表中插入 `("dispatch-op", None)`。
2. **绑定**：`bind-type :dispatch-op Op` 将 slot 值设为 `Some(Enum(Op, []))`。绑定发生在预处理阶段（通过 `resolve_program_value_for_preprocess` 提前求值），确保后续同一编译 pass 的类型检查能立即使用。
3. **解析**：当类型标注遇到 `*dispatch-op` 时，`resolve_type_slot("dispatch-op")` 返回绑定的 `Enum(Op, [])`。
4. **检查**：解析后的类型委托给标准的 `matches_with_bindings` / `value_matches_type_annotation` 进行检查。

### 约束

- 每个 slot 只能声明一次（重复声明报错）。
- 每个 slot 只能绑定一次（重复绑定报错）。
- 未绑定的 slot 在类型检查时等同于 `:dynamic`（不报错但不检查）。
- 绑定必须是 enum、struct 或 record 类型。

## 4. API 参考

### `deftype-slot`

声明一个类型占位符。

```cirru
deftype-slot :dispatch-op
```

- **参数**：1 个 tag 或 string，作为 slot 名称。
- **返回**：`nil`
- **副作用**：在全局 `TYPE_SLOTS` 注册表中注册 slot。

### `bind-type`

将具体类型绑定到已声明的 slot。

```cirru
bind-type :dispatch-op Op
```

- **参数**：
  1. tag 或 string — slot 名称（必须已通过 `deftype-slot` 声明）。
  2. enum / struct / record 定义值。
- **返回**：`nil`
- **副作用**：将类型绑定写入 `TYPE_SLOTS`。
- **错误**：slot 未声明、slot 已绑定、第二参数类型不对。

### `*name` 类型引用语法

在 schema 类型标注中使用 `*name` 引用 slot：

```cirru
;; 在 EventHandler 的 schema 中
:args $ [] '*dispatch-op
```

Cirru EDN 序列化为 `'*dispatch-op`（`'` 是 EDN symbol 前缀，`*` 是 type-slot 标记）。

## 5. 使用示例

### Respo EventHandler 场景

**库端（respo.schema）：**

```cirru
;; 声明 slot
deftype-slot :dispatch-op

;; EventHandler schema 引用 slot
:: :fn $ {} (:return :unit)
  :args $ [] 'respo.schema/RespoEvent
    :: :fn $ {} (:return :unit)
      :args $ [] '*dispatch-op
```

**应用端（app.main）：**

```cirru
;; app.schema 定义 Op enum
defenum Op
  :add :string
  :remove :tag
  :toggle :map
  :update :tag :string
  :clear
  :states-merge :any :any :any

;; main! 中绑定
defn main! ()
  bind-type :dispatch-op Op
  ;; ... 后续代码
```

**效果**：

```cirru
;; ✅ 正确 — 编译通过
d! $ %:: Op :toggle (:id task)

;; ❌ 错误变体名 — 预处理警告 "does not have variant :delete"
d! $ %:: Op :delete (:id task)

;; ❌ 载荷数量错 — 预处理警告 "expects 1 payload(s), got 2"
d! $ %:: Op :clear 42

;; ❌ 载荷类型错 — 预处理警告 "expects :string, got :number"
d! $ %:: Op :add 42
```

## 6. 实现细节

### 修改的文件

| 文件 | 变更 |
|---|---|
| `src/calcit/type_annotation.rs` | `TYPE_SLOTS` 注册表, `TypeSlot` 变体, slot 解析/匹配/序列化 |
| `src/calcit/proc_name.rs` | `DeftypeSlot` / `BindType` proc 名称 |
| `src/builtins.rs` | dispatch arms |
| `src/builtins/meta.rs` | `deftype_slot()` / `bind_type()` 实现 |
| `src/calcit.rs` | re-export `register_type_slot`, `bind_type_slot` |
| `src/runner.rs` | `clear_type_slots()` 在程序启动时调用（避免跨次运行残留） |
| `src/runner/preprocess.rs` | 预处理阶段提前执行 `deftype-slot` / `bind-type` |

### 关键实现点

1. **Thread-local 注册表**：`TYPE_SLOTS` 使用 `thread_local! { RefCell<HashMap<...>> }`，因为 `HashMap::new()` 不是 const fn，不能用 `const { ... }` 初始化。

2. **预处理时绑定**：`bind-type` 在预处理阶段通过 `resolve_program_value_for_preprocess()` 提前求值，确保同一编译 pass 内类型检查可以立即使用绑定结果。这是整个机制的关键——如果在运行时才绑定，预处理阶段的类型检查无法看到具体类型。

3. **类型匹配委托**：`TypeSlot` 在 `matches_with_bindings` 和 `value_matches_type_annotation` 中解析后直接委托给标准匹配逻辑，不引入新的匹配分支。

4. **序列化**：`TypeSlot(name)` 序列化为 `Edn::Symbol("*name")`，反序列化时 `*` 前缀触发 `TypeSlot` 解析。

## 7. 局限性与未来方向

- **单绑定约束**：每个 slot 只能绑定一次。如果需要一个库支持多个不同 dispatch 类型或一个项目中有多个不同的 EventHandler，需要声明多个 slot。
- **无运行时效果**：`deftype-slot` 和 `bind-type` 在运行时是无操作的（返回 nil），它们的作用完全在预处理阶段。
- **仅支持 enum/struct/record**：不能绑定基础类型（如 `:number`）到 slot，因为 slot 的主要场景是复合类型注入。
- **未来可扩展**：如果未来 Calcit 引入泛型或 trait 约束，type-slot 可以作为特化机制的基础。

## 8. 从 editing-history 迁移说明

本提案内容源自 `editing-history/202507131553-type-slot-mechanism.md`，已扩充为完整 RFC 格式。原始文件可在合并后删除。
