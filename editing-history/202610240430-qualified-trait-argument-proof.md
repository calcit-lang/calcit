# 保留泛型身份，同时证明全限定 trait 参数

## 问题与取舍

既有 `test-traits.main/describe-greeting` 声明 `T: Greeting`，通过方法调用返回 String；Greeting 还依赖 Labeled。普通检查及 native/JS 运行成立，但独立返回 proof 把接收者 T 与全限定的 Greeting/Labeled 参数当作无关类型。隔离项目中，仅将 trait 方法的接收者声明从短名改为全限定名，就会让完整 strict 从通过变为拒绝，运行结果不变。

约束提供方法能力，不提供新的值身份。不能把 T 替换为 trait 描述符，也不能让两个实现相同 trait 的类型互相冒充。

## 共享关系

- `CallTypeProof` 仅在实际参数是已记录能力的调用方 TypeVar 时，使用既有 `proves_trait_bound` 证明 trait 义务。
- 通过既有名义 trait 图检查自身、传递依赖及多约束；全限定引用保留 namespace 身份，并验证可达方法 schema。
- 能力不绑定调用方 T，不证明 Number、List payload、独立变量或带参数的无依据引用；callee 泛型传递仍保存原 T。
- 方法接收者先沿用原容器/参数绑定，再使用同一词法调用证明。推导、调用校验和查询复用这条关系，不新增方法名白名单或平行检查入口。
- 查询只复用已捕获的泛型能力；显示原 T 和已证明的方法合同，不借查询创建新的能力。

## 回归与边界

用户语义放在现有 describe-greeting 的附带测试，验证具体值通过自身及父 trait 方法得到 `hello Ada / Ada`。现有证明回归脚本同时检查独立 proof、查询保留 T，以及两个方法均返回 String。保留全部旧断言和原拒绝用例。

私有 Rust 回归检查 CallTypeProof 的绑定表和 trait 元数据图：能力不能改变词法身份、凭空生成类型或借给 Dynamic；缺少约束、错误 namespace、不完整多约束及损坏 schema 仍拒绝。这些内部状态无法通过稳定语言接口直接观察。

不改公开 API、诊断编号、运行时、后端协议、依赖、质量预算或迁移入口。整个历史 trait fixture 仍有其他独立证明缺口；本次局部通过不代表全库 strict 或普通检查切换已完成。反例还复现了既有 trait 描述值被误当作实现实例的独立问题（#2307）；它不能由此能力修复掩盖，需沿值/类型边界继续处理。
