# 根据真正选中的 JS 实现检查合同

真实消费者 Cumulo Reel 使用已发布的 Calcit/procs 0.30.0-alpha.2 与 JS-FFI
0.2.3。完成 Map、Option、callback、泛型与 DOM 合同修复后，完整 strict
仍报告 path-join 的 Cirru body 缺少独立 String 返回证明。query 明确显示该定义
实际执行模块内 JS file；在未执行 body 加校验不能验证真实的 node:path 调用。

处理采用通用的已选实现语义，不为 path-join 加豁免或新增迁移规则：已登记的
defn 通过原 schema/参数和 JS 元数据验证后，保留 callable declaration，JS codegen
继续选择实际 inline/file；不预处理、执行或证明未选中的 shadow body。普通
Calcit 函数和只声明 feature 的函数仍检查实际 body，调用方的参数检查保持。
JS schema 仍是作者的外部合同，不是编译器解析或运行 JS 得到的返回证明。

回归扩展现有 JS FFI harness。file 实现配上故意不合法的 shadow body，实际
生成 JS 和 strict 应通过；Unit inline 的不兼容 shadow body 与真实 Unit
返回由 fixture 的附带 Calcit 测试固定。无实现元数据的同类 Calcit 错误必须
继续拒绝；原 native/WASM/跨 target、实际模块引用、宿主失败及 source-map
验证保留。测试不增加维护入口、公共规则、计数预算或 Dynamic。

既有 FFI evidence 也只保留已选 inline/file 的声明来源和 target，不再从未选
body 推导宿主操作或推荐 helper/trait/adapter。仍明确保留 no-runtime-trust-inference，
未增加 envelope 字段、诊断编号或公开命令。

此候选基于 7dc8f6a6 的独立工作区，不改变 alpha.2 源码/tag；源代码、发布工具链
和完整消费者验收分别记录，正式配对后才能接受下游完整迁移。
