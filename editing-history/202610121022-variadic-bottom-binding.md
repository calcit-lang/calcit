# 可变参数调用保留泛型绑定

Algebra 的 `Maybe0<T>` 真实迁移暴露原有 none 结果断言被可变参数相等调用阻断。精确 main 独立构建仍拒绝 `= (%none) (%some 1)`，交换参数却通过；#1983 修复后的固定参数合同已经通过。问题在调用检查提前以首参绑定替换 rest 的原始 T，导致通用 closed-bottom refinement 看不到变量。

用户函数、局部函数、proc 和方法的既有 rest 检查保留原始泛型进行 matching；诊断仍显示当前绑定后的类型。不新增类型关系、诊断编号、proof 特性、迁移规则或检查脚本。底类型扩展是否安全仍由 #1983 的通用类型关系决定，Ref 不变量、真实 payload 不匹配和 caller-owned 变量不能被逆向推断。

回归放在已有类型推导定义的 attached tests，覆盖直接相等与局部 rest 调用；已有 strict runner 扩展原本的 payload mismatch 和 Ref 拒绝用例。Algebra 保留原有断言和值顺序，候选需重放全量 native/JS 和核心门禁。候选验证不代表正式编译器或下游已发布。关联 #1990、#1981、#1553。
