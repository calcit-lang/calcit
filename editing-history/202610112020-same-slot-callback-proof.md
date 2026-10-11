# 同一 type slot 的 callback 合同证明

## 实际阻塞与范围

Respo #195 的同-slot callback 正例被拒绝，诊断中的期望和实际都为 `fn(type-slot(dispatch-op)) -> :unit`。独立工作树从待授权的 f1f00da2797b7cc2278e43b48794e2cc001749cd 创建；原候选及冻结的路线报告保持不变。本次是未发布源码修复，尚未交付框架的完整 dispatch 类型路线。

## 原因与处理

兼容关系和严格证明原先逐端展开 slot，共用按名称记录的递归守卫。第一端展开后，第二端的同名引用再次进入守卫，合法关系因此被误判为循环。现在对同名 slot 一次读取共享绑定，在同一守卫内比较两端解析后的类型，再沿原有参数方差、返回值和泛型证明流程检查。没有用名称相等直接跳过类型证明。

未绑定 slot 的严格证明仍需要边界；自循环和已有双-slot 循环仍拒绝。显式 Dynamic opt-out 遵循正常 Dynamic 关系：Dynamic 对 Dynamic 可以匹配，不能据此证明具体 Number。原兼容模式的未绑定 fallback 保持。

## 回归放置与已有证据

- Rust 单元测试先在旧规则下失败，覆盖内部 slot registry、递归守卫、未绑定和 Dynamic 关系；这些内部绑定状态无法由正式 entry 的名义类型绑定接口完整表达。
- 用户语义放在 `calcit/test-types.cirru` 的 Person `:tests`：传入同-slot callback 并检查收到的名义值。原 checker 在 invoke 调用处误拒，修复后 native/JS 各 1/1，零排除。测试通过官方 transaction、dry-run 和 Person scoped revision 写入，并接入已有共享运行器及 check-core-tests。
- 现有 Respo 65 场景完整对照中，8 个合法 slot 正例由拒绝转为接受：三项 struct Op/额外 data，以及五项 Store/Props/callback 正例。后五项 native 回归通过。typed struct/Store/Props 的错误 Number、错 Op、错 state 仍拒绝。
- 两项未注解原始 map 的 Number 场景仍接受，旧 Tag/cursor-list slot 调用仍拒绝；它们没有因本次修改而完成验收。没有把调查当作正式下游 receipt，也没有选择生产路线。
- slot 相关 Rust 测试 18/18，all-target Clippy、TypeScript 构建通过。完整 Rust：26 suites、1687 passed、0 failed、1 ignored。

## 验证中的环境故障

第一次完整 Rust 测试的两项 HTTP fixture 因沙箱禁止本机端口绑定失败；保留失败日志后，在允许本机监听的环境中完成完整门禁。

新工作树最初整体链接旧 node_modules，生成 JS 的 procs 和本工作树 lib 属于不同模块实例，字符串转换门禁的 CalcitTag 身份断言失败。现为本工作树单独建立依赖链接，@calcit/procs 指向本工作树；原工作树依赖未修改。失败项复验通过，完整 check-all 随后退出 0。失败记录和大型 JSON 仅保留在忽略的 .calcit 目录。

完整门禁开启 CALCIT_LINT_CORE=1：Agent 64/64，core native/JS/WASM 617/616/385，保持原 1 项 JS、232 项 WASM 排除；新增同-slot definition test 在完整门禁中 native/JS 各 1/1、零排除。classification 仍为 296，IR/WASM 与技术基准完成。完整命令使用关闭增量编译的本工作树 checker，没有把失败运行或未发布源码混用计为正式依赖回归。

## 尚待完成

本轮源码门禁通过；与新合并 main 的同步仍需在独立分支完成并重新验证。Respo 完整 strict、原始 map 的 callback 上下文、旧 dispatch 调用兼容、路线反馈与正式下游依赖回归仍需独立验收。
