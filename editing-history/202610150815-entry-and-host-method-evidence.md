# 入口选择与宿主方法返回证据

## 实际问题

Rebase Hell 的 Node Chalk adapter 已通过 external-object trait 声明 String 返回，但正式 alpha.3 的语义查询仍存在两个独立问题：顶层 server 入口未传给 query loader，导致错误的 browser target 诊断；即使默认入口改为 Node，方法签名中的隐式接收者变量也未进入返回推断的调用证明，结果退化为 Dynamic。前者由 #2070 跟踪，后者由 #2071 跟踪，共同阻塞 #1916 的真实消费者迁移。

## 决策与边界

项目 type、type-at、context 使用已有入口 loader；search 和 search-expr 继承顶层入口，原局部选择优先。保留 core-only 和 metadata-only 的原读取路径，不增加命令、参数、输出字段或全局状态。

方法参数检查、静态查询合同与返回推断共享已有的严格接收者绑定。返回推断先确定可由实际接收者证明的变量集合，再将显式方法泛型与隐式 self 一起交给原 CallTypeProof，在写入任何调用方类型之前统一 freshen；随后按原签名依次证明接收者、实参、arity 和 where bounds，调用方变量保持 rigid。Review 复现了先代换再 freshen 会捕获同名调用方变量的问题，内部回归验证无输入证据的结果保持 Dynamic、接收者相关结果保持原变量、其他参数确实绑定的结果仍能精确推断。未知接收者、显式 Dynamic 返回、错误输入以及未授权 FFI 不会因此得到具体返回证据。不修改 host runtime，也不在应用加入 decoder、强转或扩大 Dynamic。

## 验证方式

语言行为扩展现有 JS FFI fixture 的 definition :tests，通过原运行器验证 prefix/postfix、继承的接收者、返回值与参数仅求值一次。CLI 边界留在原 post_lowering_cli harness，覆盖 target、入口依赖、类型槽、feature policy、无效入口、局部覆盖、EDN/JSON 单文档以及错误调用和 rigid 泛型的拒绝。类型槽测试通过已有返回合同识别两种名义绑定，不依赖额外的字段推断能力。

真实 Chalk adapter 用候选编译器复核，源程序不改；候选结果不冒充已发布的 CLI/runtime 配对。完整 fmt、clippy、Rust、Agent interface 与 CALCIT_LINT_CORE 全量检查，以及最新 HEAD review/CI 和精确 main 验证仍按原门槛执行。正式配对发布与消费者完整验收尚未完成，不能据此关闭稳定 0.30.0。
