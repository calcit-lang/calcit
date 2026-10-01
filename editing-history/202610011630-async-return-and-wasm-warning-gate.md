# async 尾返回与 WASM 告警门禁

关联 #1538、#1533；复用现有函数返回证明、pending async 类型及 CLI 检查，不新增 analyzer、类型标签或公开入口。

## 问题与决策

WASM 生成收集预处理告警却继续发射产物，检查模式又可能先返回目标 unsupported。直接阻断暴露已有 Component async fixture 的误报：声明 Result/Bool/String 的 async export 尾调用 async import，推断值是内部 pending async 表示，而返回契约描述完成后的结果。

只在函数已有 async invocation 标记时，从内部 pending 表示取出逻辑结果，然后使用同一 TypeProof 关系检查。同步函数保留 pending 表示，不能凭 Number 返回声明伪装成已完成值；async 结果类型不一致仍拒绝。普通参数的 await 要求不变，不引入自动等待、不扩大 Dynamic。

WASM/WASI 在发射前统一报告并拒绝预处理告警；check-only 先完成已有范围的预处理和告警检查，再验证目标。把 WASI/Component 重复的 namespace 预处理合为一次。fixture 中两处 count 参数改为 item-count，仅消除名字遮蔽，保持位置与运行行为。

## 验证设计

正例定义在 hint-fn 的 :tests，由现有 runner 在 native 与真实 JS 回放；同一表达式另作为临时函数工厂，真实 JS 宿主等待其结果验证 Promise 转发。同步函数返回 pending 值、async 结果类型错误为编译负例；原有参数与返回负例在 WASM/WASI 的生成与检查模式下也必须报告源类型错误，不得以 unsupported 代替。拒绝后不得写入 program.wasm。

现有 Component 的真实 Node/Wasmtime 测试继续验证 import 结果、取消、句柄生命周期、HTTP 与流，不以静态文本检查代替。全量 Rust、Clippy、格式和现有集成测试的最终结果以 PR 为准。本步不完成所有 Dynamic 证明义务，也不实现新的 async 控制流或 quote 支持。

全量回放已暴露旧 test-wasm fixture 的 Bool→Number 伪声明、可空 first/last 直接算术与 to-pairs Set/List 混用；先修正五个 Bool schema，并为 Bool not 保留原生 :tests。其余债务及 async 负例的生成函数 source 映射尚未解决，PR 保持 Draft，不降低 gate 或更改预期值以合并。
