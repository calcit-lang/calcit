# `defimpl` 方法 hint 的 `Self` 隐式绑定

`defimpl Type Trait` 的内联方法体里，`hint-fn` 中的 `Self` 现在绑定为实现类型，省略 `(:generics Self)` 与显式写出行为一致。实现方式是在预处理 `&impl::new` 方法对时，复用 `with_implicit_self_generic` 改写方法值的 hint schema，补齐 `(:generics Self)`；`deftrait` 与 `defimpl` 因此共用同一种 `Self` 表达。支持 compact `Fn`、canonical `(:: 'Fn …)` 与 `hint-fn` 实际携带的 schema map 形式。

边界保持不变：`(:impls …)` 仍是显式能力声明，`defimpl` 仍须用户提供的顶层定义名，实现方法仍须显式标注签名；方法值以符号引用独立 `defn` 时，hint 写在该 `defn` 内，不享有隐式绑定。普通 `defn`、`assert-type` 等其他 schema 里的 `Self` 仍报 `E_UNKNOWN_TYPE_NAME`。本次为纯增量改动，现有写 `(:generics Self)` 的 Snapshot 不需要迁移。

验证：`calcit/test-traits.cirru` 的 `SelfBindingBoxImpl` 覆盖 `Fn Self (-> String)`、`Fn Self (-> Self)` 与 `Fn Self (-> (List Self))`；Rust fixture 锁定普通 `defn` 仍报错。`cargo fmt`、`cargo clippy -- -D warnings`、目标测试与 `CALCIT_PROOF_AUDIT=0 CALCIT_LINT_CORE=1 yarn check-all` 均通过。
