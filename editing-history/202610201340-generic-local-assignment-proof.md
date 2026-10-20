# 局部泛型赋值保持原 binding 合同

关联 #2187、#2121、#1553。未发布基线的 `Fn<T>(T)->T` 可执行
`set! value ([] |wrong)` 后返回 value；普通、独立 proof、完整 strict 与 JS emit 都会通过，
真实 JS 对 List<Number> 输入返回 String 成员。赋值不能被当作可重新实例化 T 的泛型调用。

在已有赋值校验处，原 binding contract 优先于临时可读类型；复用 caller-rigid CallTypeProof，
不给赋值声明 callee inference variables。普通与 strict 不保留两套许可，使用原错误编号和 source
定位。其后的 refinement 撤销与 callable 写入追踪保持不变，不改变公开 API、宿主赋值运行时、
求值次数或 Option payload。

Number、List<Number>、Option false 的同型赋值语义放在 test-js definition :tests，实际 JS 回放。
错 payload 和独立 U→T 的拒绝进入已有 known-assertion harness，覆盖普通、独立、完整 strict 和
JS emit，并验证源码未改。不新增 checker 或语言语义 Rust 测试。此补丁不批准 #2213 的终点 API
变化，也不完成 #2187 尚存的 kind→T 和 core 泛型返回合同；不影响0.30.1维护线。
