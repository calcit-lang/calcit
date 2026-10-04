# EDN 根节点与布局参数的后端一致性

Diary 存储边界的 native/JS 共享回放暴露了 Number 根节点在 JS 中被错误送入 list writer 的问题；补上既有标量包装，不增加平行解析或类型入口。

PR 审查又确认 native 的 builtin 签名接受可选 Bool，但实现始终使用 true。标量的布局原本不随参数改变，因此仅重复标量测试不能证明 false 的容器布局；新增嵌套 List 测试断言两种文本不同、默认等价 true、解析结果相同。原标量测试保留。

native 读取已声明的参数并保留默认 true；native/JS 宿主边界拒绝非 Bool，不以 truthiness 放宽类型。可由语言表达的 round-trip 与布局语义都放在 definition `:tests`，Rust/JS 的低层测试只验证绕过预处理的宿主参数拒绝。

WASM 的现有闭合类型编码器没有非行内容器布局。标量输出不受布局参数影响，保留参数求值；容器只接受省略或字面量 true，其他模式明确拒绝，而不延续静默忽略。新增 `E_WASM_EDN_FORMAT_MODE` 区分“类型已闭合但布局能力未实现”，既有 EDN type/depth/shape 错误不能准确表达这个状态。不扩展 WASM 动态类型探测或新编排入口。

验证以修复前共享 native 测试失败为基线，再用当前源码的新 CLI 与重编译 JS 回放。现有 runner 在真实 WASM 中回放 definition 上支持子集的默认/true 容器布局、两种标量布局及固定文本断言；容器 false/运行时 Bool 由代码生成拒绝，且无产物。临时 Snapshot 经 revision 守卫与 dry-run 事务修改，不新增脚本或并行解析接口。完整 Rust/跨平台门禁由最新 PR HEAD 的 Actions 验证，不能借用更新前全绿记录。非有限 Number 文本策略不在本次改变。
