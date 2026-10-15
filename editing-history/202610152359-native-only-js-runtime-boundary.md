# native-only helper 的 JS 运行时边界

## 决策依据

#2093 的真实 Graphviz 回放在 JS 发射成功后才报 missing-function TypeError。`&format-ternary-tree` 读取 native List 的内部布局，没有对应 JS 实现；为此复制内部布局实现或新增 feature、命令、迁移规则都不合适。

将此 helper 纳入现有 core 定义省略列表，复用 compiled `DefId` 依赖。从 init/reload 两个运行时入口检查 macro 展开后的引用；遇到该 native-only proc，沿用代码生成栈报告所属定义，并在模块写入前失败。quoted proc 是符号数据，不能当作运行时调用。旧省略列表不等于完整的运行时目标矩阵：`&reset-gensym-index!` 等已有 JS warning stub，本修复不顺带改变它们的行为。

Snapshot 发射会补齐未使用的源码定义，所以不能不分可达性地拒绝整个模块。仅省略 unavailable 定义及依赖它们的定义，其余未使用定义仍按原方式输出；不引入全局 dead-code elimination。宏自身不输出到 JS，其 native helper 可以继续在展开阶段执行。已成功求值并消失的常量不产生不存在的运行时调用。

## 兼容边界

原来能发射但在 JS 运行时报错的调用提前失败。正常模块 API、宏展开、native helper、原类型合同及 WASM 支持范围不变。内部调试 helper 的定义文档说明 native-only，并区分 portable `format-cirru-edn` 的值显示语义。

## 验证方式

在既有 JS namespace CLI 边界测试中创建真实 Calcit 模块，覆盖直接/限定/跨模块/first-class 引用、init/reload、宏间接调用 native helper、无关普通 API 的实际 Node 执行和只读源码。参数保持运行时未知，结果被实际打印和断言，避免仅测到静态类型或常量折叠。Rust 只负责进程退出、源码归属和生成文件边界；表达式与断言仍使用 Calcit。

结合现有 core native/JS 同源回放、完整 cargo test/check-all、Clippy 和不可变依赖编译验收，不新增排除、不降低严格类型要求。
