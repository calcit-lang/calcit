# 类型用 `(:impls …)` 列出 trait，`defimpl Type Trait` 沿引用挂载（#2727）

## 决策

- Jon 在 2026-11-12 确定了 trait 重新设计的三点：在编译期挂载，不写第二个 `def X (impl-traits …)`；实现只能写在类型或 trait 的 ns 里；顶层自动挂载，`let` 里的局部定义继续用 `impl-traits`。
- 之后 Jon 补充了一条规则：snapshot 上的定义按引用载入，不做全量扫描，实现也必须沿引用找到。最初的实现是扫描全部 `defimpl`，因此改成：类型在 `(:impls Trait …)` 里列出 trait，编译器只在类型所在的 ns 和 trait 所在的 ns 里查找 `defimpl 类型 trait`。

## 实现

- `src/program/impl_attach.rs` 在 `extract_program_data` 和 `apply_code_changes` 中运行。它去掉类型代码里的 `(:impls …)`，改写成 `(calcit.core/impl-traits <定义> <实现引用>…)`。
- 因为生成的就是已有的 `impl-traits` 形状，类型推断、`:where` 约束检查和各后端都不需要改动。唯一的改动是 `type_annotation::is_impl_traits_head` 同时识别限定符号 `calcit.core/impl-traits`。
- 写入时的原始代码存放在 `WRITTEN_CODE`。重新挂载前，只有当前代码仍等于上次生成的代码时才还原；被热重载替换过的定义保留新代码。
- 热重载时比较挂载前后的代码，有变化的类型及其传递依赖会失效重建，所以在 trait 的 ns 里改 `defimpl` 也会让类型重编。
- `data-definition-clauses` 收到 `(:impls …)` 时报错，因为这个子句只会出现在 `let` 等局部定义里。

## 错误码

- `E_MISSING_TRAIT_IMPL`：列出的 trait 在两个 ns 里都没有实现。写在第三个 ns 的实现会落到这里。
- `E_DUPLICATE_TRAIT_IMPL`：同一个 trait 有两个实现。
- `E_UNKNOWN_IMPLS_TRAIT`：`(:impls …)` 里的名字解析不到。

## 验证

```bash
cargo test --lib -- defimpl_type_trait reload_reattaches
calcit calcit/test-traits.cirru            # test-traits.source-impl / source-gauge
calcit calcit/test.cirru --no-proof-audit js && node js-out/main.mjs
```
