# WASM HOF Intercepts & Set Intersection

## 修改概要

将 WASM codegen 的不支持 proc 数量从 72 减少到 68（减少 4 个）。

## 核心问题

calcit.core 内部定义了 `map/filter/each/any?/every?/find/find-index` 等 HOF，其实现体使用 `f` 参数作为调用头——在 WASM codegen 中无法静态解析局部变量作为调用目标。解决方案：在**调用点拦截**（call-site intercept），在 `emit_call_expr` 中直接 emit 内联循环。

## 文件修改

### `src/codegen/emit_wasm/hof.rs`

新增以下函数（通过 `cat >>` 追加到文件末尾）：

- **`emit_unary_step`** — 对单参数 HOF callee 发出调用（支持 Static/Inline/Proc 三种 callee 类型）
- **`emit_binary_step_ei`** — 对 `(elem, idx)` 双参数 callee 发出调用（用于 map-indexed）
- **`resolve_unary_callee`** — 解析单参数 HOF callee，返回 `FoldlCallKind`
- **`emit_map`** — `map xs f`：对每个元素调用 f，返回等长新列表
- **`emit_map_indexed`** — `map-indexed xs f`：对每个 (elem, idx) 调用 f
- **`emit_each`** — `each xs f`：对每个元素调用 f（副作用），返回 nil
- **`emit_filter`** — `filter xs f`：过滤满足 f 的元素，返回新列表
- **`emit_any`** — `any? xs f`：任意元素满足 f 返回 1.0，否则 0.0
- **`emit_every`** — `every? xs f`：全部元素满足 f 返回 1.0，否则 0.0
- **`emit_find`** — `find xs f`：返回第一个满足 f 的元素，否则 nil
- **`emit_find_index`** — `find-index xs f`：返回第一个满足 f 的元素索引，否则 -1.0

### `src/codegen/emit_wasm/sets.rs`

新增：

- **`emit_set_intersection`** — `&set:intersection a b`：返回两集合共有元素。逻辑与 `emit_set_difference` 对称，条件取反（`I32Ne` 代替 `I32Eq`）。

### `src/codegen/emit_wasm/lists.rs`

修改 **`emit_list_concat`**：从只支持 2-arg 改为支持任意数量 args（通过左折叠依次合并）。新增私有辅助函数 **`emit_list_concat_two`** 处理 2-arg 快速路径。

### `src/codegen/emit_wasm.rs`

**`emit_call_expr` 的 `calcit.core` 拦截块** 新增：

```rust
"map" if args_list.len() == 2 => return emit_map(ctx, &args_list),
"map-indexed" if args_list.len() == 2 => return emit_map_indexed(ctx, &args_list),
"each" if args_list.len() == 2 => return emit_each(ctx, &args_list),
"filter" if args_list.len() == 2 => return emit_filter(ctx, &args_list),
"any?" if args_list.len() == 2 => return emit_any(ctx, &args_list),
"every?" if args_list.len() == 2 => return emit_every(ctx, &args_list),
"find" if args_list.len() == 2 => return emit_find(ctx, &args_list),
"find-index" if args_list.len() == 2 => return emit_find_index(ctx, &args_list),
"concat" if args_list.len() >= 2 => return emit_list_concat(ctx, &args_list),
"deref" if args_list.len() == 1 => return emit_expr(ctx, &args_list[0]),
```

**`emit_proc_call`** 新增：

```rust
CalcitProc::NativeSetIntersection => emit_set_intersection(ctx, args),
```

**`emit_call_spread`** 新增 `Calcit::Proc` arm：

```rust
Calcit::Proc(proc) => emit_call_spread_args_as_regular(ctx, proc, call_args),
```

新增辅助函数 **`emit_call_spread_args_as_regular`**：收集 spread 调用中的真实参数，跳过 ArgSpread 标记，转交 `emit_proc_call` 处理。

## 关键设计理解

### 调用点拦截 vs 定义编译

- calcit.core 的 HOF 定义（如 `calcit.core/map`）内部使用 `f` 参数作为调用头，**定义本身无法编译**（会 skip）。
- 拦截在 `emit_call_expr` 的 `Calcit::Import` arm，当用户代码调用 `(map my-list my-fn)` 时触发，直接 emit 内联循环，**无需编译 calcit.core/map 的定义体**。
- 因此 skip 列表中仍可见 `calcit.core/map`（定义 skip），但用户代码的调用点正常生成 WASM。

### 剩余 68 个 skip 的分布

- **核心库定义失败**（~66 个）：使用运行时 spread（`& args`）、方法调用（`.deref`）、嵌套 defn、'f' 作为调用头等无法在 WASM 编译时静态解析的特性。
- **用户代码 skip**（2 个）：`recollect.app.comp.panel/on-click` 和 `recollect.test/test-diff-funcs`，均因内部使用了嵌套 defn（nested lambda）。

## 测试结果

- `yarn test:wasm`：**全部通过**（=== Recollect WASM checks passed ===）
- skip 数量：72 → 68（减少 4 个）
