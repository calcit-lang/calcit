# with-type: 局部作用域类型绑定

日期：2026-06-09

## 功能概要

实现 `with-type-slot` 表达式，允许在局部代码块范围内绑定 type-slot，替代全局 `bind-type` 的全局副作用。

## 语法

```cirru
with-type-slot (:slot-name TypeExpr)
  body-expr1
  body-expr2
  ...
```

- 第一个参数必须是二元列表 `(:slot-name TypeExpr)`
- 后续参数是 body 表达式
- 返回最后一个 body 表达式的值

## 与 bind-type 的对比

| 特性 | bind-type | with-type-slot |
|------|-----------|-----------|
| 作用域 | 全局（整个预处理 pass） | 局部（仅 body 范围内） |
| 是否可重叠 | 单次绑定，不可重复 | 可嵌套，栈式覆盖 |
| 使用位置 | 通常在 main! 最顶部 | 任意代码块 |

## 实现细节

### 修改文件

1. **`src/calcit/type_annotation.rs`**
   - 新增 `TYPE_SLOT_OVERRIDES` thread-local（HashMap of Vec）
   - 新增 `push_type_slot_override(name, ty)`：压栈一个作用域覆盖
   - 新增 `pop_type_slot_override(name)`：弹栈，自动清理空栈
   - 修改 `resolve_type_slot`：优先检查 override 栈，再回退到基础 `TYPE_SLOTS`
   - 修改 `clear_type_slots`：同时清理 override 栈

2. **`src/calcit/proc_name.rs`**
   - 添加 `WithType` 枚举变体（strum serialize = "with-type"）
   - `get_type_signature` 返回 `None`（type-checking 在预处理阶段处理）

3. **`src/builtins.rs`**
   - 添加 `WithType => meta::with_type_runtime(args)` dispatch

4. **`src/builtins/meta.rs`**
   - 添加 `with_type_runtime()` 运行时 no-op（返回最后一个 body 值）

5. **`src/runner/preprocess/mod.rs`**
   - 新增 `preprocess_with_type_block()` 函数：
     - 解析 binding pair → 解析类型注解
     - push override → 预处理 body → pop override（错误时也 pop）
   - 在 `preprocess_list_call` 中：在 generic `Calcit::Proc` 分支之前插入 `WithType` 特殊分支

6. **`src/calcit.rs`**
   - re-export `push_type_slot_override`, `pop_type_slot_override`

### 运行时行为

`with-type-slot` 在运行时是 no-op：类型绑定仅影响预处理阶段的类型检查，不影响运行时值。运行时返回最后一个 body 表达式的值。

### 预处理时机

`with-type-slot` 的 body 在 override 激活期间被预处理。由于 `ensure_ns_def_compiled` 是懒加载的，所有从 body 中引用的 def（包括传递依赖）都会在 override 有效时被编译，从而获得正确的类型检查。

## 验证

使用 calcium-workflow 项目验证：将 `app.client/main!` 从 `bind-type` 改写为 `with-type-slot`：

```cirru
;; 改写前
defn main! () (bind-type :dispatch-op Op)
  ...

;; 改写后
defn main! ()
  with-type-slot (:dispatch-op Op)
    ...
```

运行 `cr calcit.cirru --check-only` 验证通过：`✓ Check passed (59ms)`
