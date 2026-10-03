# Countable 合同与名义参数的能力证明

## 依据

升级工作流收紧开放值后，`count` 的旧无约束泛型 schema 仍允许无法计数的输入通过预处理。这里把现有 Countable 能力写入合同，不增加场景化诊断、迁移规则或动态追踪注册表。此改动面向 0.29 非 patch 版本，与 #1694 的真实迁移边界相关。

把合同用于顶层名义参数时，最小复现显示 TypeRef 被当作声明的 StructDef/EnumDef 元数据，未获得值实例已有的能力。通用关系现在复用现有的 source-backed 名义解析，并保留类型参数与声明来源。能力可以证明实例满足 trait，不能反向证明 trait 值是具体实例；同名用户 trait 也不能借用 core trait 的能力。

## 边界

- List/Map/Set/String 的长度、Struct 字段数、Enum 包含 tag 的计数保持不变。String 使用 Unicode 标量单位。
- 泛型须声明 Countable；Dynamic 使用内容前显式收窄。nil、Number 和函数不能作为可计数值。
- defstruct 字段验证先确认列表形状再计数，不伪造其动态输入 helper 的返回类型。
- 内部 map destruct 与 map diff 返回异质内容，附带测试保留原断言，分别在 Map/List 谓词成功分支计数。
- 不插入 native count，不生成默认值、unsafe 或新的自动 fix。本任务不是 #1553 的通用改写后 verifier。
- 负例发现普通预处理跳过显式 Dynamic 参数的声明，将体内需要的能力反推为参数类型。修复为始终把声明合同放入 body scope，与 proof audit 保持一致；类型谓词成功分支仍可正常收窄。这一通用变更必须重新完成全量及真实消费者验证，不能只凭 workflow 的 schema-dynamic 审阅项失败宣称 Countable 已强制。
- 同名调用方/被调用方泛型可在 compatibility 关系中通过而不产生 binding。where 检查现在要求实际能力证明；缺失 binding 作为未证明的变量处理，不再静默跳过约束。沿用 W_GENERIC_WHERE_BOUND_MISMATCH，不增加逐函数特例。
- 约束实参从原声明恢复，复用 CallTypeProof 分离调用方与被调用方变量，避免把合法的 Set<T> 或含泛型方法的 trait 判为递归变量。同一 core 回归还暴露了局部 hint-fn 的 quoted trait 经 macro 展开为 quote 表达式后被误当作多约束列表的问题；修复解析，不改原 ToString 测试预期，并加入局部 Countable 正例。
- 空泛型 enum 的无载荷分支没有足够信息证明 payload 的 Debug 能力；已有测试仅给该字面量补上 Maybe1<Number> 类型上下文，保留原调用与结果断言。这不是把 Dynamic 强转为具体类型。
- Respo 已提交 main 的副本验证发现 typed loop 初始化经过异质参数列表后丢失各位置的类型。最小用例加入 fixture 的附带测试；立即调用的固定参数函数从实际输入接收上下文，canonical apply 的无 spread 字面量参数列表降为同顺序的直接调用。显式函数 hint 优先，非字面量列表沿用原路径，不为 Respo 名字增加特例。原 Respo lis-values 测试已恢复通过，完整消费者验收仍待其余真实边界修复。
- apply 定义的附带测试还验证不同位置的输入、空集合类型和函数/各实参的求值顺序。已有大型 case 回归发现新增上下文临时量增大递归栈帧；将它们移出普通调用路径，保留原大型测试并恢复通过，不提高栈限制或减少分支。
- 全量 JS 运行还发现语句位置的函数 literal 被生成成 declaration 后接独立实参表达式，副作用循环未执行。JS emitter 把函数 callee 保持为表达式，并在有实参 prelude 时先求值复杂 callee；保留原累计结果 6 的断言，同时把独立循环副作用加入 native/JS 附带测试回放。
- 显式 Dynamic 参数现在按动态 schema 诊断，而不是误分类为缺少 schema；相关测试保留原拒绝行为并检查准确的来源和收窄提示。迁移回归中的非法 Dynamic 转换现在独立检查预览失败与 Snapshot 不变，再验证其余合法迁移，不能把其悄悄推断成 ToString 来取得成功。

## 验证方案

用户语义置于 count 及 fixture 的定义附带测试：空集合、集合、Unicode、直接名义构造器、顶层名义参数和 typed rest。扩展现有跨后端运行器，回放这些表达式到 native/JS，并拒绝无能力及无约束输入，检查 Snapshot 不被改写。Rust 测试只覆盖内部来源、关系方向与参数保留。

合并前还需完整 core/native 测试、全量集成回归、格式化、clippy、Cargo 测试、Agent 协议检查和真实消费者副本验证；执行结果记录在 PR，不以此计划冒充验收完成。
