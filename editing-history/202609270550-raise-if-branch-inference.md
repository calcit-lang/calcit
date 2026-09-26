# `if` 的直接 `raise` 分支不参与返回类型合并

关联 [#1411](https://github.com/calcit-lang/calcit/issues/1411)。在 0.24.2 中，`if` 一支返回 `Option<Number>`、另一支直接 `raise` 时，普通分支合并把结果退成 `Dynamic`，导致 `option:unwrap` 报 `E_DYNAMIC_NOMINAL_ARGUMENT`。js-ffi 的类型化宿主错误路径遇到了同样的限制。

这次只复用预处理器已有的 `expression_definitely_diverges` 判定：直接 `raise` 不产生可参与合并的值，结果采用另一支的推断类型。普通 `Dynamic` 分支仍参与原有合并，未证明发散的调用、嵌套表达式和异常可能性不被当作 bottom。没有改动求值、错误抛出或 backend lowering，也没有新增静态检测规则。

验证以 `calcit/test-types-inference.cirru` 中 definition `:tests` 为主，覆盖左右两种分支顺序、`Option<Number>` 泛型消费、实际抛错路径；Rust 单元测试只补预处理器分支判定与普通 `Dynamic` 不被错误收窄的低层边界。另用编译失败夹具确认未发散的 `String` 分支仍不能传入 `Option` 合约。运行 native、JS 和完整仓库门禁确认共同语义。
