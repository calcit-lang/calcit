# Enum payload 的声明命名空间

真实消费者的递归 ContentNode 使用短名时，普通检查与 native／JS 执行通过，独立返回证明却将 ContentNode 与完整路径判为不同类型。只改成完整路径即可通过，说明这是类型身份解析丢失，不是需要放宽的开放值边界。

构造检查、构造器泛型推导和 match 绑定共用声明侧的命名空间解析。先解释 Enum 声明中的短名，再代入调用方的泛型实参；不能反过来把调用方类型绑定到声明侧的同名类型。不修改 Snapshot 表示、公共 API、运行时值或诊断编号，不增加 Dynamic／unsafe。

同名且字段相同的错误 Struct payload 负例进一步揭示：独立证明原本跳过具体构造器的 payload 合同，只检查底层原语的开放传递签名。修复这个既有入口，复用相同的定向调用证明；普通检查的开放值政策保持不变。不能仅证明返回值属于某个 Enum，就把其 payload 输入视为已验证。

共享语义写在 calcit/test-enum.cirru 的 definition :tests 中，覆盖递归 pair／List、空 List、声明侧 Struct 与调用侧同名类型，以及泛型实参保留调用侧身份。复用既有运行器回放 native、JS、WASM 和可选 WASI，既有 known-assertion harness 同时验证独立证明。不新增 checker 或固定改写。

该修复独立于 Sliceable 终点合同的待决方案；不带入尚未完成的 CodeEntry 全局绑定原型，也不修改 0.30.1 维护线。完整 CI、最终提交审查和精确 main 验证仍是合并后的交付门槛。
