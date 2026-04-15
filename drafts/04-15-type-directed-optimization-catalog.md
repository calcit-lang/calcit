# 类型导向优化机会目录

## 背景

随着 `&record:nth` 的 O(1) 索引重写（commit `2fd2776` 及后续 JS field-tag 修复）落地，Calcit 预处理阶段已具备"在类型已知时改写 AST 以提升运行效率"的基础设施。本文档系统梳理各数据结构的现状与可做的同类优化，供后续逐项推进。

## 数据结构现状

| 类型 | 内部表示 | 查询复杂度 | 更新复杂度 | 已有编译期优化 |
|------|---------|-----------|-----------|--------------|
| Record | `Vec<Calcit>` + 字段按字母排序的 `CalcitStruct` | O(log n) 二分 | O(n) clone Vec | `&record:nth` 索引重写 ✅ |
| Map | `rpds::HashTrieMapSync` | O(1) hash | O(1) persistent | 无 |
| List | `Vec` / `TernaryTreeList` 自动切换 | O(1) 或 O(log n) | O(1) prepend/append | 结构自动选择 |
| Tuple | tag + `Vec<Calcit>` | O(1) index | O(n) clone | enum variant HashMap 查找 |
| Set | `rpds::HashTrieSetSync` | O(1) hash | O(1) persistent | 无 |
| Scope | `TernaryTreeList<ScopePair>` | **O(n) 线性扫描** | O(1) push | 无 |

## 优化项目

### P0: Tag 调用 Record 的运行时 fallback

**问题**: `(:field x)` 在 runner.rs 只处理 `Calcit::Map`。当 `x` 是 `Calcit::Record` 时直接报错 `"expected a hashmap"`。预处理阶段的 `(:tag record)` → `&record:nth` 重写仅在类型已知时生效；类型未知时走 fallback 触发运行时错误。

**位置**: `src/runner.rs` L318-336，`Calcit::Tag(k)` 分支。

**方案**: 在 Map 分支后增加 Record 处理：

```rust
} else if let Calcit::Record(record) = &v {
    Ok(record.get(k.ref_str()).cloned().unwrap_or(Calcit::Nil))
}
```

**性质**: 正确性修复，非纯性能优化。是渐进式类型策略的前提。

**影响**: 高 — 允许 `(:field x)` 在类型未知时也能正确运行。

---

### P1: `&record:assoc` 编译期索引化

**问题**: `&record:assoc record :field value` 运行时做 `index_of` 二分查找定位字段，然后 clone 整个 `Vec<Calcit>` 再改一个位置。

**位置**: `src/builtins/records.rs` L1020-1055。

**方案**: 模式与 `&record:nth` 完全一致 — 预处理阶段类型已知时，算出字段索引，emit `NativeRecordAssocAt(record, idx, :tag, value)`：

- 新增 `CalcitProc::NativeRecordAssocAt` variant
- `proc_name.rs` 添加签名 `(record, number, tag, any)`
- `records.rs` 添加 `record_assoc_at` 运行时函数（跳过 `index_of`，直接 `values[idx] = value`）
- `emit_js.rs` 添加 codegen（JS 端同样省掉运行时字段查找）
- `preprocess/mod.rs` 在检测到 `NativeRecordAssoc` + 已知类型时做重写

**收益**: 每次 Record 字段更新省一次 O(log n) 二分查找。

**影响**: 中高 — Record 更新在 Cumulo updater 和 Respo state 中是高频操作。

---

### P2: `&record:with` 批量更新索引化

**问题**: `&record:with record :a 1 :b 2` 每个字段都做一次 `index_of` 二分查找。

**位置**: `src/builtins/records.rs` L630-720。

**方案**: 类型已知时，编译期预计算所有字段索引，emit 一个携带 `[(idx, value)]` 的批量更新指令。

**收益**: k 个字段更新从 k×O(log n) 降到 O(k)。

**影响**: 中 — 构造新 Record variant 或批量 state 更新时受益。

---

### P3: `tag-match` 分支整数化

**问题**: `tag-match` 运行时用 tag 字符串逐一比较（线性扫描分支）。JS codegen 也是 if-else 链。

**位置**: `src/builtins/syntax.rs` L933-1050（runner），`src/codegen/emit_js.rs` L897-978（JS codegen）。

**方案**:
- **Rust 端**: enum 类型已知时预处理阶段把 tag 比较替换为 `variant_index` 的整数比较
- **JS 端**: emit `switch (tag.idx)` 替代 if-else 链

**收益**: 5+ 分支时 O(n) → O(1)。

**影响**: 中 — 状态机、消息路由场景（Cumulo updater dispatcher）明显加速。

---

### P4: Method dispatch 静态绑定

**问题**: `.method obj` 运行时路径：① 匹配 receiver 类型 → ② 查 impl 列表（builtin 还要 evaluate symbol）→ ③ 线性遍历 `impls` 数组找方法名。

**位置**: `src/builtins/meta.rs` L1016-1095，`method_call_impls` 函数。

**方案**: 预处理阶段 receiver 类型和 trait 均已知时，直接解析到 `CalcitFn`/`CalcitProc`，把 `.method obj args` 重写为 `(resolved-fn obj args)`。

**收益**: 消除 symbol resolution + linear impl search。

**影响**: 中 — `.map`, `.filter`, `.show` 等核心 API 全部走 method dispatch。

---

### P5: Scope 变量查找 O(1) 化

**问题**: `CalcitScope` 用 `TernaryTreeList<ScopePair>` 存变量，lookup 向后线性扫描 O(n)。每次变量引用（每个表达式节点）都付出这个代价。

**位置**: `src/runner/fns.rs` L157-197。

**方案**:
- **方案 A**: 预处理阶段把 `Local.idx`（u16）做 de Bruijn 风格的 slot 编号（当前已有 `CalcitLocal::track_sym` 给全局唯一 idx），运行时 scope 改为 `Vec<Option<Calcit>>` 直接索引
- **方案 B**: 保持 persistent tree 但加一层 `HashMap<u16, usize>` 缓存位置

**收益**: 变量查找从 O(depth) 降到 O(1)。

**影响**: 高 — 变量查找是最热操作，对解释器整体吞吐量有根本性影响。需要仔细验证 shadowing、closure capture、persistent scope push 等语义是否兼容。

---

### P6: `get-in` / `assoc-in` 静态路径展开

**问题**: `get-in base [:a :b :c]` 是 Calcit 编写的递归函数（`calcit-core.cirru`），每层递归拆列表 + 动态 `get`。

**方案**: 路径是字面量列表且 base 类型已知 Record 时，编译期展开为嵌套 `&record:nth`。`assoc-in` 同理展开为嵌套的 `&record:assoc-at`。

**收益**: 消除递归、list 分解、运行时字段查找。

**影响**: 中低 — 频率中等，但 Cumulo updater 中 `assoc-in db [:users user-id :field] value` 是核心模式。

---

### P7: `if` 条件常量折叠

**问题**: `if true x y` 运行时仍求值条件。

**方案**: 预处理阶段条件是字面量 `true`/`false`/`nil` 时直接消除分支。

**影响**: 低 — 手写代码少见，但宏展开后常见。

---

## 实现模式参考

所有 Record 相关优化遵循 `&record:nth` 已验证的模式：

1. `proc_name.rs` — 新增 `CalcitProc` variant + `ProcTypeSignature`
2. `records.rs` — 新增运行时函数（接受已解析的索引参数）
3. `preprocess/mod.rs` — 检测原始 proc + 类型已知时重写 AST
4. `emit_js.rs` — 添加 codegen 分支（JS 可能需要不同策略，参考 `&record:nth` 的 field-tag 方案）
5. `type_checking.rs` / `type_inference.rs` — 确保新 proc 参与类型检查

## 推荐执行顺序

**近期**: P0 → P1 → P5（正确性 → 高频操作 → 热路径）

**中期**: P3 → P4（enum/trait 优化，需类型覆盖率提高后收益更大）

**长期**: P2, P6, P7（递减收益，视实际瓶颈决定）
