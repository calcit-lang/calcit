# 上下文名义构造的字段校验

## 问题与决策

Respo UI 的 AttributesOptions 示例将已知 Number 放入 Option<Number> 字段。函数参数类型触发 Map→Struct 上下文 lowering，但改写发生在参数预处理之后，生成的构造器没有走普通构造器的字段检查。外层名义类型检查不能替代实际字段证明，导致默认严格检查通过，native/JS 在运行时以不同错误失败（#1778、#1529）。

保留现有上下文构造语义：不删除合法 Map/loose Struct 简写，不改变缺字段补 nil 的现有行为；补出的 nil 同样必须满足字段契约。函数实参完成上下文改写后，仅对实际改写的构造器复用 `check_struct_construction_fields`，与源码构造器共享形状、payload 及 source 位置检查。泛型实参关系继续由现有函数调用校验负责。没有新增 diagnostic、analyzer、命令或 Dynamic 例外，不改写业务默认值，也不改变字段求值顺序。

这属于 #1553 改写后证据校验；当前加入既有回归集，不另建校验系统。实际 JS 已经生成 Struct 构造，并非完全保留 Map；原 issue 的 JS 描述以生成代码核对后更正。

## 验证

Calcit definition `:tests` 覆盖显式泛型/具体构造、上下文 Map 构造和 Option some/none；现有已知断言运行器在 native 和真实生成 JS 重放同一组测试。严格负例覆盖泛型/具体错误字段、错误 Option payload、缺必需字段、未知字段、具体泛型参数不匹配与 loose Struct 错误字段，并检查拒绝时没有应用 JS 产物且 Snapshot 不被改写。

同时回放 Respo UI 的最小失败源与显式 AttributesOptions 正例，执行 Rust、原有 native/JS/WASM 集成、Clippy 与 Agent CLI 门禁。此处只扩大当前已知断言运行器的覆盖，不新增测试计数或另一套编译入口。
