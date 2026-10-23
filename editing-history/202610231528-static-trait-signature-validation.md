# 在预处理阶段验证普通 trait 方法签名

#2128 的 Editor 迁移反馈暴露了一个诊断差异：`deftrait` 的完整方法签名包含 `Dynamic` 时，普通检查先报告通过，运行时的 `&trait::new` 才拒绝声明。调用方还可能先收到无法解析 trait 的次生错误。

把运行时已有的方法签名规范化、解析和校验提取为共享函数，并在预处理 `&trait::new` 的静态字面量方法列表时调用。保留内部 quote，避免冷态 WASM 检查将 `Option<Number>` 等名义应用误当作 Dynamic。只检查被 quote 的签名元数据；计算生成的元数据仍由运行时校验，预处理不执行用户代码。沿用原 Type 错误文本，并从原签名或构造调用取得 source location，不新增诊断编号。

JS external-object 的属性和方法由原 FFI 合同处理。判定复用同一个定义元数据函数，且从签名的 source 节点识别 owning CodeEntry；依赖声明可能在调用方的 stack frame 下编译，不能用调用方名称替代声明拥有者，也不能仅凭整个函数带有 js-ffi feature 就豁免。

本次不改变方法签名允许的类型集合；完整签名、无完整合同的 Fn、泛型模型与 JS FFI 仍按既有边界验证。#2128 的更广 trait 设计与 Anchor 模块交付继续独立跟踪。

编译拒绝回归加入现有 trait 边界脚本，覆盖多种 Dynamic 嵌套形状、native/JS/WASM/WASI 编译入口，以及未被主入口引用的声明在 namespace、all-defs 和完整 strict 检查中的结果。合法方法语义沿用现有 Calcit definition tests 与真实 JS 回放；额外回放 JS FFI 以核对宿主边界。
