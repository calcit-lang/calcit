# async 尾返回与 WASM 告警门禁

关联 #1538、#1533；复用现有函数返回证明、pending async 类型及 CLI 检查，不新增 analyzer、类型标签或公开入口。

## 问题与决策

WASM 生成收集预处理告警却继续发射产物，检查模式又可能先返回目标 unsupported。直接阻断暴露已有 Component async fixture 的误报：声明 Result/Bool/String 的 async export 尾调用 async import，推断值是内部 pending async 表示，而返回契约描述完成后的结果。

只在函数已有 async invocation 标记时，从内部 pending 表示取出逻辑结果，然后使用同一 TypeProof 关系检查。同步函数保留 pending 表示，不能凭 Number 返回声明伪装成已完成值；async 结果类型不一致仍拒绝。普通参数的 await 要求不变，不引入自动等待、不扩大 Dynamic。

WASM/WASI 在发射前统一报告并拒绝预处理告警；check-only 先完成已有范围的预处理和告警检查，再验证目标。把 WASI/Component 重复的 namespace 预处理合为一次。fixture 中两处 count 参数改为 item-count，仅消除名字遮蔽，保持位置与运行行为。

## 验证设计

正例定义在 hint-fn 的 :tests，由现有 runner 在 native 与真实 JS 回放；同一表达式另作为临时函数工厂，真实 JS 宿主等待其结果验证 Promise 转发。同步函数返回 pending 值、async 结果类型错误为编译负例；原有参数与返回负例在 WASM/WASI 的生成与检查模式下也必须报告源类型错误，不得以 unsupported 代替。拒绝后不得写入 program.wasm。

现有 Component 的真实 Node/Wasmtime 测试继续验证 import 结果、取消、句柄生命周期、HTTP 与流，不以静态文本检查代替。全量 Rust、Clippy、格式和现有集成测试的最终结果以 PR 为准。本步不完成所有 Dynamic 证明义务，也不实现新的 async 控制流或 quote 支持。

全量回放暴露旧 test-wasm fixture 的 Bool→Number 伪声明、可空 first/last 直接算术与 to-pairs Set/List 混用。五个 Bool schema 已纠正；对于已由固定输入或非空分支保证存在的元素，使用正常 `.first` / `.last` 与 Option `.unwrap`，保留原有期待值，增加 definition :tests 覆盖列表、排序、空列表递归与首字符。该修改是人工核对输入后的 fixture 修复，不作为可对任意应用自动插入 unwrap 的 fix。

to-pairs 的 runtime 返回 Set<List>，不是原 schema 声称的 Set<Enum>。同步 Rust proc signature 与 core schema，保留集合及内部 List 形状；只有异质键值槽位保留 Dynamic，与现有 Map.to-list 边界一致，读取具体内容仍需窄化。新增 core :tests 检查真实形状和普通方法调用，分类清单明确记录该既有表示限制，不把元数据纠错描述成动态边界已消除。

async 负例的生成函数 source 映射复用 CallTypeCheckInfo 与已有 macro source 查找，在末尾表达式无位置或只有 gen% 位置时回到调用源。专项 runner 已通过 native/JS 正例、真实 Promise adoption、标量 WASM 与全部类型负例；不降低 source-context 断言。

固定参数 to-string 的 spread 负例现在先被共同预处理拒绝，专项测试明确检查该告警、源定义和失败阶段，不能继续要求被新门禁抢先阻止的 WASM lowering 错误。全量回归和远程 CI 最终结果以 PR 为准，未通过全部门禁前保持 Draft。

本地完整 yarn check-all、Rust、all-target/all-feature Clippy、fmt 与增加 Set<Enum> 拒绝案例后的专项 runner 通过。完整 WASI preprocess 脚本也通过，包括此前 Linux CI 停止的真实文件 starter；本地成功不等于解释了远程失败。保留 Draft 等新 HEAD 验证，给原静默 starter grep 增加实际输出诊断，不改变期待值或跳过断言。
