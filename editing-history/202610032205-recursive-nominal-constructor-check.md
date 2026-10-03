# 递归名义字段的构造检查

关联 #1734、#1553，真实消费者为 Respo #194 的 RenderNode / ChildPair 递归模型。

## 决策

constructor-head 在改写前检查未预处理的参数，嵌套构造尚未形成完整名义证据；改写成 `&%{}` 后的通用 proc 检查又跳过字段合同。显式 `%{}` 同样绕过该检查。因此合法递归声明能够执行，但错误的 Struct -> Enum 字段和 List<List<...>> -> List<ChildPair> 也被接受。

把已有字段兼容检查移到 `NativeStruct` 参数预处理之后，两种表层写法共享同一检查。保留声明身份、字段求值顺序、递归遍历和开放字段，不新增 Respo 特判、类型 registry、诊断编号或自动 fix。复用已有类型关系和 `W_FN_ARG_TYPE_MISMATCH`，保留期望/实际类型及调用来源。

具体字段使用已有 `is_proven_for` 关系，而不是仅询问是否兼容。广义 `Struct` 与某个具体名义声明可以不矛盾，但前者无法证明后者的身份；这种 `NeedsBoundary` 不能当作构造成功。显式 Dynamic 字段仍允许开放值，具体字段要求既有的 decode/narrow 证据，不新增另一套类型规则。

空集合复用已有 `empty_container_has_no_type_evidence`。Enum 复用分支合并已有的 variant 泛型参与检查，只从已编译的直接构造或函数末尾构造取得变体证据；仅补全该变体未使用的泛型槽。已有的 `%ok` 等薄 wrapper 同样按其已编译函数体判断，不按名字授予权限，也不把实际参与 payload 的 Dynamic 补成具体类型。Nullable 字段仍按既有 nil / 内层值表示检查，不强迫改成 Option，不将字段存储检查误当作 nullable 向非 nullable 的缩窄。

此处不改变声明 Dynamic 字段的合法传递，也不把静态检查当作外部数据校验；未解析的动态 prototype 仍属于原有运行时边界。

真实 Respo 全量回放还暴露了兼容关系的方向问题：混合 TypeRef / 已实例化 Struct 或 Enum 的 symmetric compatibility arm 可能反向检查泛型参数。早期候选曾修改该 arm，但严格构造检查改用已有定向 proof 后移除了这项额外改动。保留合法的 `Option<Element> -> Option<Struct>` 和泛型 Struct 广义字段，同时拒绝广义 Struct 反向证明特定名义类型；附带测试覆盖两类容器，不另扩展 compatibility 规则。

## 验证方式

现有 `check-known-assertion.mjs` 创建 Calcit 定义及附带测试，覆盖合法递归 variant matching、空 Component 和三种 key 值，回放 native 与真实生成 JS。严格负例覆盖两种构造写法的标量和递归字段错误、错误 Enum payload 与匹配分支字段访问，在 native/check/JS/WASM/WASI 入口均要求拒绝且不产生应用产物。替换旧的直接调用 rewrite helper 的 Rust 字段错配测试，让公共语言调用成为验证对象。

独立 Respo 调查副本对照当前 main 与候选编译器；不改动用户 checkout 的未提交 RenderNode 设计。递归模型不因此宣称已经在 Respo 生产树或所有 WASM 路径完成迁移。
