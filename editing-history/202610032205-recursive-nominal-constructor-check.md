# 递归名义字段的构造检查

关联 #1734、#1553，真实消费者为 Respo #194 的 RenderNode / ChildPair 递归模型。

## 决策

constructor-head 在改写前检查未预处理的参数，嵌套构造尚未形成完整名义证据；改写成 `&%{}` 后的通用 proc 检查又跳过字段合同。显式 `%{}` 同样绕过该检查。因此合法递归声明能够执行，但错误的 Struct -> Enum 字段和 List<List<...>> -> List<ChildPair> 也被接受。

把已有字段兼容检查移到 `NativeStruct` 参数预处理之后，两种表层写法共享同一检查。保留声明身份、字段求值顺序、递归遍历和开放字段，不新增 Respo 特判、类型 registry、诊断编号或自动 fix。复用已有类型关系和 `W_FN_ARG_TYPE_MISMATCH`，保留期望/实际类型及调用来源。

此处不改变声明 Dynamic 字段的合法传递，也不把静态检查当作外部数据校验；未解析的动态 prototype 仍属于原有运行时边界。

真实 Respo 全量回放同时暴露了混合 TypeRef / 已实例化 Struct 或 Enum 比较的方向问题：原 symmetric match arm 总是从 TypeRef 参数比较另一侧参数，在 TypeRef 是期望值时错误反向检查。修正实际值到期望值的方向，保留合法的 `Option<Element> -> Option<Struct>` 和泛型 Struct 广义字段，不把广义 Struct 反向证明为特定名义类型。附带测试覆盖两类容器。

## 验证方式

现有 `check-known-assertion.mjs` 创建 Calcit 定义及附带测试，覆盖合法递归 variant matching、空 Component 和三种 key 值，回放 native 与真实生成 JS。严格负例覆盖两种构造写法的标量和递归字段错误、错误 Enum payload 与匹配分支字段访问，在 native/check/JS/WASM/WASI 入口均要求拒绝且不产生应用产物。替换旧的直接调用 rewrite helper 的 Rust 字段错配测试，让公共语言调用成为验证对象。

独立 Respo 调查副本对照当前 main 与候选编译器；不改动用户 checkout 的未提交 RenderNode 设计。递归模型不因此宣称已经在 Respo 生产树或所有 WASM 路径完成迁移。
