# WASM codegen: test derived type predicates on top of `type-of`

## 背景

刚在上一条 commit 里把 `type-of` 跑通了，语料里 `calcit.core` 的几个核心
predicates (`list?` / `map?` / `number?` / `set?` / `tuple?` / ...) 都是
`(defn pred? (x) (&= (type-of x) :tag))` 这种薄包装。既然 `type-of`、`&=`、
keyword 字面量都已经支持，这些 predicate 本来就会被 WASM 编译器吐出来，
只是一直缺少验证。

## 改动

`calcit/test-wasm.cirru` + `scripts/test-wasm.mjs` 增加 4 个样例：

- `test-list?-true` : `(list? ([] 1 2))` → 1
- `test-list?-false` : `(list? 42)` → 0  （验证魔数检查能挡住整数伪装）
- `test-number?-true` : `(number? 42)` → 1
- `test-map?-true` : `(map? (&{} :a 1))` → 1

`yarn check-all` 全部通过，81 个 WASM 检查全绿。

## 当前 WASM skip 分类（跑 `cr wasm` 的 stderr）

总计 123 条 skipping，主要集中在：

- 24 String values not yet supported
- 19 nested defn not supported
- 13 foldl / foldl-shortcut / foldr-shortcut
- 7 `&call-spread`
- 4 `Tuple operation %:: not yet supported`
- 3 `&set:destruct`
- 少量其它（`sort`, `range`, `println`, `deref`, `turn-string` 等）

下一轮切入点优先级：字符串 → nested defn/闭包 → 高阶函数 (`call_indirect`)。
