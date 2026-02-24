# 202602242030 项目全面审查报告

> 审查日期: 2026-02-24 | 版本: 0.11.6 | Binary: 4.4MB | 全测试耗时: ~100ms

## 一、本次修复的问题

### 1. `&str:contains?` 类型签名错误 (已修复)

`&str:contains?` 实际接受 `(string, number)` (检查索引是否越界)，但类型签名声明为 `(string, string)`，与 `&str:includes?` 共享了错误的签名。

- 文件: `src/calcit/proc_name.rs` L692
- 影响: `cr eval` 中使用 `&str:contains?` 时会触发假阳性类型警告，导致 eval 失败
- 修复: 将 `NativeStrContains` 和 `NativeStrIncludes` 的签名分离

### 2. 文档 Bug 修复 (4处)

| 文件                                              | 问题                                                             | 修复                                                                 |
| ------------------------------------------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------- |
| `guidebook/docs/intro/overview.md`                | 引用了错误的 `im` crate，实际使用的是 `rpds` + `im_ternary_tree` | 改为正确的 crate 名和链接                                            |
| `guidebook/docs/features/sets.md` L52,56          | `difference` 和 `intersection` 示例中有多余的 `}`                | 删除多余字符                                                         |
| `guidebook/docs/features/list.md` L36             | `&doseq (x xs) (println a)` 变量名拼错                           | 改为 `(println x)`                                                   |
| `guidebook/docs/features/static-analysis.md` L241 | `defstruct Point :x :y` 缺少类型标注、`(%:: ...)` 语法错误       | 改为 `defstruct Point (:x :number) (:y :number)` + `(%{} Point ...)` |

### 3. 过时 Drafts 归档 (7个文件)

已移入 `drafts/archived/`:

- `last-session.md` — 使用了已废弃的 `%%::` 语法
- `macro-trait-improvements-checklist.md` — 全部任务已完成
- `record-struct-and-enum-plan.md` — 全部任务已完成
- `record-struct-and-enum-rfc.md` — 已归档设计文档
- `202602182334-panic-hardening-summary.md` — 已完成的 session log
- `202602192322-thread-macro-and-sort-followup.md` — 已完成的 session log
- `202602201114-record-tuple-reflection-api-rename.md` — 已完成的 session log

### 4. Eval 测试套件

新增 `js-out/test-eval-examples.sh`，包含 133 个 `cr eval` 测试用例，覆盖:

- 算术/逻辑 (16) · 比较 (5) · 字符串 (20) · 列表 (25) · 高阶函数 (11)
- Map (10) · Set (9) · Tuple (4) · 控制流 (7) · 线程宏 (2) · Atom (4)
- 类型操作 (10) · 位运算 (6) · 元操作 (6)

---

## 二、`cr eval` 验证中发现的关键知识点

### Cirru 空字符串语法

```
|     → 空字符串 (长度 0)
||    → 字符串 "|" (长度 1)
|hello → 字符串 "hello"
```

可通过 `cr cirru parse -e '...'` 验证任何表达式的解析结果。

### `->` 线程宏与 CLI 参数冲突

`->` 会被 shell 解析为后台重定向。在 `cr eval` 中使用时需要 `--` 分隔:

```bash
# 正确
cr eval -- '-> (range 5) (map inc)'
# 错误 (-> 被 shell 消费)
cr eval '-> (range 5) (map inc)'
```

### `contains?` vs `includes?` 语义区分

| 类型   | `contains?`          | `includes?`          |
| ------ | -------------------- | -------------------- |
| list   | 检查**索引**是否有效 | 检查**值**是否存在   |
| map    | 检查**键**是否存在   | 检查**值**是否存在   |
| set    | (无)                 | 检查元素是否存在     |
| string | 检查**索引**是否越界 | 检查**子串**是否包含 |

### 不存在 `flatten` builtin

`flatten` 在 `calcit-core.cirru` 中定义为 `&list:flatten`，不是 builtin proc。通过 `cr eval` 使用时需要加载包含标准库的 compact 文件:

```bash
cr demos/compact.cirru eval '&list:flatten ([] ([] 1 2) ([] 3 4))'
```

替代方案: `apply concat ([] ([] 1 2) ([] 3 4))`

### `round`/`floor`/`ceil`/`negate`/`abs` 是裸名函数

数学函数不带 `&number:` 前缀: `round 3.7` ✓, `&number:round 3.7` ✗

带前缀的数学 proc: `&number:rem`, `&number:fract`, `&number:format`, `&number:display-by`

---

## 三、代码设计审查发现

### A. 性能问题 (建议优化)

1. **Number hashing 有碰撞风险**
   - `src/calcit.rs` 中 `(*n as usize).hash(state)` 将 f64 转 usize，丢失小数和负数信息
   - `1.5` 和 `1.9` 会 hash 到相同值
   - 建议: 改用 `n.to_bits().hash(state)`

2. **`run_fn` recur 中冗余 `.to_vec()`**
   - `src/runner.rs`: `Calcit::Recur(xs) => current_values = xs.to_vec()` — `xs` 本身已是 `Vec<Calcit>`
   - 每次 recur 多一次无谓的堆分配

3. **`CalcitList::drop_left` 在 Vector 表示上是 O(n)**
   - `src/calcit/list.rs`: 每次函数调用 (`call_expr`) 都会走 `xs.drop_left()`
   - 若列表以 `Vector` 形式存在，每次调用都付 O(n) 代价
   - 建议: 加载后立即转换为 `TernaryTreeList`

4. **`buffer_bit_hex` 每字节分配一个 Vec**
   - `src/calcit.rs`: `hex::encode(vec![n])` → 建议用 `format!("{n:02x}")`

### B. 代码重复 (可合并)

1. **`foldl_shortcut` / `foldr_shortcut` ~200行重复**
   - `src/builtins/lists.rs`: 相同的 tuple 解构逻辑在 List/Set/Map 三种类型上各重复一次
   - 可提取为参数化 helper

2. **`run_fn` 和 `run_fn_owned` 近乎相同**
   - `src/runner.rs`: 两个函数仅参数类型 `&[Calcit]` vs `Vec<Calcit>` 不同
   - recur 循环整体复制，可统一

3. **`CalcitRecord::index_of` 和 `CalcitImpl::index_of` 重复**
   - 两处相同的二分查找实现，应共享

4. **一元数学函数模板化**
   - `floor`/`ceil`/`sqrt`/`round`/`sin`/`cos` 模式完全相同，可用宏生成

### C. 正确性风险

1. **`PartialEq` 跨 variant 相等**
   - `src/calcit.rs`: `Symbol == Local` 和 `Symbol == Import` 在 name 相同时返回 `true`
   - 违反 enum variant 不等性预期，在集合操作中可能导致微妙 bug

2. **`identical?` 回退到值相等**
   - `src/builtins.rs`: Calcit 的 `identical?` 实际调用 `binary_equal`（值相等）
   - 对 `Arc` 包装的类型可用 `Arc::ptr_eq` 实现真正的引用相等

3. **非测试代码中的 `unwrap()`**
   - `src/data/cirru.rs`: `s.chars().next().unwrap()` 空字符串会 panic
   - `src/builtins/meta.rs`: `entry.first().unwrap()` / `entry.get(1).unwrap()`
   - `src/runner/preprocess.rs`: 多处 `args.get(N).unwrap()`
   - 建议统一改为 `expect("reason")` 或返回 `Result`

4. **`println!` 用于警告违反 IO 纯净性**
   - `src/runner.rs` L203: `println!("[Warn] macro should already be handled...")`
   - 应全部改为 `eprintln!`

### D. 架构观察

1. **`preprocess.rs` 4512 行单文件**
   - 同时处理符号解析、宏展开、类型检查、方法推断、arity 验证
   - 建议拆分为子模块: `symbol_resolution.rs`, `type_inference.rs`, `macro_expansion.rs`

2. **`Calcit` enum 27 个 variant**
   - 大多数运行时值是 `Nil/Bool/Number/Tag/Str/List`
   - 低频 variant (`Struct`, `Enum`, `Trait`, `Impl`, `Macro`, `Buffer`, `CirruQuote`) 可用 `Box` 间接引用，减小 `size_of::<Calcit>()`

3. **全局 `RwLock` 状态**
   - `IMPORTED_PROCS`, `program.rs` 中的多个 `RwLock` 静态变量
   - 阻止同进程运行多个 Calcit 程序，每次符号查找都获取读锁

### E. API 命名不一致

| 模式                     | 示例                                       |
| ------------------------ | ------------------------------------------ |
| `&type:method` (单冒号)  | `&list:nth`, `&map:get`, `&str:count`      |
| `&type::method` (双冒号) | `&struct::new`, `&enum::new`, `&impl::new` |
| 裸名                     | `floor`, `ceil`, `sort`, `range`, `not`    |
| `&method`                | `&+`, `&-`, `&=`, `&buffer`, `&hash`       |

单冒号 vs 双冒号的区别未文档化。建议在 guidebook 中补充命名约定说明。

---

## 四、文档缺失清单

| 优先级 | 缺失内容                                                | 建议                                         |
| ------ | ------------------------------------------------------- | -------------------------------------------- |
| P0     | `defenum` 完整语法无专门文档                            | 新增 `features/enums.md`                     |
| P0     | `tag-match` vs `&case` 区别未说明                       | 在 quick-reference 或 common-patterns 中补充 |
| P1     | `field-match` 无示例                                    | 补充                                         |
| P1     | `->%` 和 `%<-` 线程宏无说明                             | 补充示例                                     |
| P1     | `foldl-shortcut` / `foldr-shortcut` 的 tuple 协议未解释 | 补充 `(:: true result)` 提前退出约定         |
| P2     | `&buffer` 操作缺少专门章节                              | 新增或扩充                                   |
| P2     | `&doseq` 语法无专门文档                                 | 补充                                         |
| P2     | MCP server (`cr-mcp`) 在 intro 中提及但无文档页         | 新增                                         |
| P2     | `apply-args` 模式                                       | 补充                                         |
| P3     | `cr eval` 上下文加载机制                                | 解释 `demos/compact.cirru` 提供了什么        |
| P3     | 方法分发优先级 (builtin vs user vs `&trait-call`)       | 补充                                         |

---

## 五、现有活跃 Drafts 状态

| 文件                                | 状态                             | 建议                                             |
| ----------------------------------- | -------------------------------- | ------------------------------------------------ |
| `runtime-traits-plan.md`            | Phase 1 完成, Phase 2/3 进行中   | **核心参考文档**, 保持更新                       |
| `assert-types-plan.md`              | Phase 1-3 完成, Phase 4 部分完成 | 需对照当前代码重新评估 remaining tasks           |
| `assert-types.md`                   | 大量已实现内容混合未来计划       | 建议拆分为「已实现参考」+「未来计划」            |
| `project-modernization-roadmap.md`  | Milestone A 完成, B 接近完成     | Milestone C (benchmarks, workspace split) 仍待做 |
| `generics-struct-fn-proc-plan.md`   | 未开始实现                       | 泛型设计参考, 保留                               |
| `language-theory-evolution-plan.md` | laws 已回滚, 纯愿景文档          | 保留但标注纯理论性质                             |
| `optional-record-macro-plan.md`     | 短提案, 无进展                   | 保留, 优先级待定                                 |
| `register-platform-api-rfc.md`      | Phase A 维持现状                 | 低优先级, 保留                                   |

---

## 六、集成测试状态

`yarn check-all` 通过。以下测试文件单独运行时失败 (需要 `util.core` 等依赖):

- `test-cond.cirru`, `test-doc-smoke.cirru`, `test-edn.cirru`, `test-fn.cirru`, `test-nil.cirru`, `test-string.cirru`, `test-tuple.cirru` — 依赖 `util.core/inside-eval:` 或 `util.core/log-title`
- `test-macro.cirru` — `-> expects symbol/tag/fn/method or list step, got: (&syntax &)`
- `test-method-errors.cirru`, `test-method-validation.cirru` — 预期错误的负面测试
- `test-invalid-tag.cirru`, `test-tag-match-validation.cirru` — 解析失败 (预期行为)
- `test-js.cirru` — JS-only 代码，解释器下不可运行
- `test-nested-types.cirru`, `test-optimize.cirru` — 可能需要更新

这些失败在 `yarn check-all` 的集成测试中已被正确处理，但建议:

1. 在文件头部注释标明哪些是「预期失败（负面测试）」
2. `test-nested-types.cirru` 和 `test-optimize.cirru` 可能有实际 bug，需排查

---

## 七、语言设计建议

### 值得考虑的补充

1. **`abs` 函数**: 标准库中有 `negate` 但没有 `abs`。建议新增:

   ```
   defn abs (x) (if (< x 0) (negate x) x)
   ```

2. **`flatten` 提升为 builtin**: 当前是 core 库函数，递归调用有性能开销。高频场景下可考虑 Rust 侧实现。

3. **`&number:clamp`**: 常见的 `(max lo (min hi x))` 可内置为 `clamp x lo hi`。

4. **Set 缺少 `contains?`**: 其他集合类型都有 `contains?` + `includes?` 双 API，Set 只有 `includes?`。

### 值得关注的设计一致性

1. **`keys` 返回 Set 而非 List**: `keys ({} ...)` 返回 `HashSet`，对需要排序的场景不方便（`sort` 接受 list 不接受 set）。Clojure 的 `keys` 返回 seq（惰性列表）。建议要么允许 `sort` 接受 set，要么提供 `sorted-keys` 便捷函数。

2. **方法链与 `->` 互操作**: `.method` 语法和 `->` 线程宏可以配合，但文档未充分说明融合使用模式。

---

## 八、性能基准

| 指标                             | 当前值                 | 目标               |
| -------------------------------- | ---------------------- | ------------------ |
| Release binary 大小              | 4.4MB                  | < 5MB ✓            |
| `cr calcit/test.cirru -1` 总时间 | ~100ms                 | ~100ms ✓           |
| `cr eval '+ 1 2'` 冷启动         | ~5ms                   | < 10ms ✓           |
| `cargo test`                     | 14 passed, 0 doc-tests | 建议补充 doc-tests |
| `cargo clippy`                   | 0 warnings             | 0 warnings ✓       |
