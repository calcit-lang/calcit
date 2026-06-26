# Simplify generic core defns via `.method` dispatch

## 概要

将 `calcit-core.cirru` 中的泛型 defn（`assoc` / `contains?` / `count` / `empty` / `empty?` / `filter` / `first` / `get` / `includes?` / `nth` / `rest` / `map`）从多分支 `if (list? x) ... if (map? x) ... if (record? x) ...` 链简化为 `list?` 快速路径 + `.method` 动态分发。

## 动机

- 用户要求：优先使用 `.method` 这套已有概念承担多态分发，避免每新增类型（record / tuple / set 等）都要在每个 defn 中追加分支。
- 所有 built-in 类型已在 `&core-*-methods` 中注册了对应的 `.assoc` `.count` `.empty?` `.get` `.nth` 等方法条目，编译期 `try_inline_method_call` 已能在静态类型已知时把方法调用内联为直接 proc 调用，runtime 则由 `invoke_method` 处理。这使得 defn 自身不再需要枚举类型。

## 实现

以 `empty?` 为例：

```cirru
defn empty? (x)
  if (nil? x) true $ if (list? x) (&list:empty? x) (.empty? x)
```

保留 `nil?` 与 `list?` 两个前置分支，其余类型全部交给 `.empty?`。

**为什么保留 `list?` 快速路径**：
`ensure_ns_def_compiled(CORE_NS, &init-builtin-impls!)` 在预处理期会展开 `do` 等 macro，其中 `(empty? body)` 会在 impls 还未注册到 runtime 时被调用；若此时 `empty?` 体里就走 `.empty?` → `invoke_method` → `evaluate_symbol_from_program("&core-list-impls", ...)` 会命中 quick-path 失败（循环依赖），导致 panic：

```
preprocess builtin impls: CalcitErr { kind: Var, msg: "expected symbol `&core-list-impls` from path `calcit.core`, this is a quick path, should succeed", ... }
```

`list?` 本身只用 `(&= (type-of x) :list)` 等 proc，不依赖 impl 注册，能在 bootstrap 期安全使用，打破循环；其余类型的 methods 此时已构建完毕，可顺利走方法派发。

## 波及范围

- `src/cirru/calcit-core.cirru`：12 个 defn 的 body 瘦身。
- 预处理期的 `try_specialize_polymorphic_call`（已有）照常将 `(assoc x k v)` 等静态可推断调用折叠为 `&list:assoc`/`&map:assoc` 等 proc。
- runtime 侧未变：仍由 `invoke_method` + `&core-*-impls` 完成最终派发。

## 验证

- `cargo fmt`
- `cargo clippy --release -- -D warnings`
- `cargo test --release`：179 + 67 通过
- `yarn check-all`：全部 WASM/JS/解释执行测试通过
- recollect (`cr --entry test` + `cr --entry test js` + `node test.mjs`) ✓
- respo (`cr --check-only` + `cr js` + `yarn vite build`) ✓

## 备注

- 下一步可以考虑：把那些只剩 `list?` 单分支的 defn 再收拢到一个运行时 primitive，或在 preprocess 阶段对 macro 展开期调用的 `empty?` 等函数强制内联 proc，从而彻底去掉这条 fast path。
