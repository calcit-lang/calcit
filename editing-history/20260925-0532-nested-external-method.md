# 嵌套外部对象方法保留 trait 类型证据

关联 [#1353](https://github.com/calcit-lang/calcit/issues/1353)。Timegrass 的 Dayjs adapter 用 `external-object` trait 声明 `.with-year`、`.with-week`，局部绑定形式能映射到 JS 方法名，嵌套形式却生成动态 `invoke_method`，在真实实例上报错。

最小复现以 JS String 的 `toUpperCase`、`toLowerCase` 模拟链式 host 方法：`TestStringChain` 将 `.upper` 映射到 `toUpperCase`，返回同一 trait；`.lower` 映射到 `toLowerCase`，返回 String。修复前嵌套 `.lower (.upper base)` 通过严格预处理，却生成 `invoke_method("lower", base["toUpperCase"]())` 并在 Node 运行失败；把中间结果存为局部变量时生成直接 `upper["toLowerCase"]()` 并成功。

根因是 `resolve_type_value` 对局部变量会把命名 trait `TypeRef` 解析成 trait，但对嵌套表达式推断的返回类型没有做同样的规范化。统一在表达式推断出口做这一步，使两种写法使用相同的类型证据；不新增动态回退、FFI 特例或额外规则。测试放在 Calcit JS fixture 中，用实际 JS 入口执行两种表层写法，因为 host 方法不能由 native evaluator 运行；生成结果和 Node 运行同时验证名称映射。

验证：针对 `test-js.main/test-nested-external-chain` 的严格预处理通过；修复前生成 JS 的嵌套调用在 Node 报 `No method '.lower'`，修复后生成 `base["toUpperCase"]()["toLowerCase"]()` 并通过，局部绑定形式也通过。`yarn try-js`、`yarn check-all`、`cargo test --lib`（859 项）、`cargo test --bin calcit`（385 项）、`cargo clippy -- -D warnings` 和 `cargo fmt --check` 均通过。使用本分支编译器检查 Timegrass draft #101 仍是原先的 1 failed、5 blocked、1 cascaded；没有重新出现已修复的依赖方法误报，其余应用层迁移单独推进。
