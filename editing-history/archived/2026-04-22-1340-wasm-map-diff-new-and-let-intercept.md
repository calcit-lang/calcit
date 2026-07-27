# 202604221340 - WASM map diff-new fix & let intercept

## 问题与修复

### 1. `&map:diff-new` 参数顺序错误 (`maps.rs`)

**问题**: `emit_map_diff_new` 中 `args[0]` (b) 和 `args[1]` (a) 搞反，导致"b 中不在 a 里的条目"变成"a 中不在 b 里的条目"。结果 `patch-map` 的 map-splice 分支从来不添加新键，`recollect` API roundtrip 测试 `summary=41` 而不是预期的 44。

**修复**: 交换 a/b 的加载顺序——先加载 `args[0]` 为 `b`，`args[1]` 为 `a`，迭代 b 检查每个 key 是否在 a 中缺失。

### 2. `calcit.core/let` WASM 拦截 (`emit_wasm.rs`)

**问题**: `calcit.core/let` 以宏形式存在，正常编译会由预处理器展开为 `&let`，但作为 Import 出现在 call position 时 WASM codegen 找不到对应函数而报错。

**修复**: 
- 在 `emit_call_expr` 的 `calcit.core` 拦截块中新增 `"let"` case
- 新增 `emit_let_multi` + `emit_let_pairs` 辅助函数，将 `(let ((name val)...) body...)` 格式转换为逐层绑定

### 3. `{}` 空 map 字面量 WASM 支持 (`emit_wasm.rs`)

**问题**: `{}` 作为裸表达式（非 call head）时，出现为 `calcit.core/{}` 的 Import 或 `CalcitProc::NativeMap`，WASM codegen 报 "unsupported WASM expression"。

**修复**:
- 在 `emit_expr` 的 `Calcit::Import` 分支加入 `def == "{}"` 的检查，调用 `emit_map_new(ctx, &[])`
- 在 `emit_expr` 新增 `Calcit::Proc(CalcitProc::NativeMap)` case，调用 `emit_map_new`

### 4. recollect probe 函数 Cirru 语法修复

**问题**: 7 个调试用 probe 函数缺少 `$` 运算符，导致 `let`/`&map:count`/`if` 等出现为裸表达式而非调用头：
- `probe-nested-*` (4 个): `let ...` → `$ let ...`  
- `probe-*-map*` (3 个): `&map:count ...` → `$ &map:count ...`
- `probe-nested-changes-count`: fn 内部 `if` → `$ if`

**修复**: 用 `cr edit def --overwrite` 重写全部 7 个函数的 Cirru 语法。

## 测试结果

- `yarn run:wasm:api` (recollect): `api-roundtrip summary=44 expected=44 OK` ✓  
- probe 函数全部通过 WASM 编译并返回正确结果：  
  `probe-empty-map=0, probe-map-count-1=1, probe-assoc-simple=2`  
  `probe-nested-bonus=3, probe-nested-count=10, probe-nested-map-count=2`
- WASM skip 数量: 62 → 51（recollect namespace 中全部消除，仅剩 calcit.core HOF/closure/variadic 等已知不支持项）

## 剩余已知 skip 分类 (51 个)

| 原因 | 数量 | 说明 |
|------|------|------|
| `'f` (动态调用头) | 26 | HOF 参数作函数调用，需函数表支持 |
| nested defn | 9 | 函数内部嵌套 defn（闭包），不支持 |
| `(&syntax &)` | 3 | 可变参数 `&`，str/str-spaced 已有拦截 |
| sort | 1 | `&list:sort-by` 需要比较函数回调 |
| recur arity | 1 | `conj` 内部尾递归 arity 不匹配 |
| 特殊 proc | 2 | tagging-edn、&core-number-impls |
