# 函数体注解与定义合同使用同一套证据

## 问题

CodeEntry 的 Fn 与函数体 Map 描述同一个函数。此前 preprocess 优先整份使用 body hint，导致 return-only 注解清掉原参数、泛型与 trait 证据；反过来，body hint 也能把公开 Number 当作 String，或自行声明调用方没有承诺的 ToString 能力。普通与 strict 一起通过并不能证明这些调用安全：实际 native/JS 会失败。

另一个同类反例是固定第二参数被注解为 rest：单纯 Fn 子类型关系能接受公开的两个实参，但函数体把实际 String 当作 List，JS 的 nth 随后失败。

## 设计

保留字段是否实际出现的信号。只为缺省字段继承 CodeEntry 的已声明值；显式 Dynamic 不是缺省。feature 合集与既有 async 调用语义不变。

在这些声明用于 body 参数推导之前，复用 caller-rigid CallTypeProof 校验完整 Fn 关系。trait 能力只从原公开合同进入证明，不能从待验证的 body 约束自行得到。显式参数与 rest 布局复用既有 definition/schema 形状校验，不增加 checker、诊断类别、函数名规则、unsafe 或 Dynamic 放宽。

兼容的两参数 hint 仍保留，其未知签名问题及 CodeEntry 泛型全局身份工作不在本批次宣称完成。隔离对照确认不同名字泛型的旧 strict 失败在基线已存在，不以本改动关闭那些任务。

## 验证与边界

合法的 return-only bounded 参数、rest 参数与单份完整匹配的 body hint 写在现有 trait-bound fixture 的 definition :tests，并复用 native/JS 共享运行器。非法声明复用 known-assertion harness，覆盖普通、独立返回 proof、完整 strict、native 和 JS emit，并验证源码未改。

原 public-return 负例保留原源码及拒绝要求；新校验更早报告 E_SCHEMA_DEF_MISMATCH，因此精确检查新的合同诊断、源位置与源码不变，不把任何编译失败当成通过。

完整回归中的 try handler 也保留原非法源码：零参数函数不能靠 hint 发明一个参数。此用例从仅独立 proof 拒绝加强到普通与 JS 编译也拒绝，并精确检查 E_SCHEMA_REQUIRED_ARGS 的 1/0 差异。位置复用已有宏调用来源恢复，指向项目定义而非内部 calcit.core/fn；不扩大整组负例的通用诊断匹配。

直接定义与宏来源使用 namespace/definition 身份区分：已有精确坐标属于调用方时保留，只有原位置属于另一处宏定义时才恢复调用方。复审确认 List 本身没有 get_location，因此改用已有 list-call 定位函数；public-return 精确断言从定义名 @1 改到 query/tree 确认的实际 hint 调用 @3，与整组 try 负例一起验证，避免粗略宏位置遮蔽直接源码坐标。

复审另一个反例中，第一份 body hint 与 Number 合同一致、第二份改为 String，原实现仍全部通过检查并只使用第一份。采用每个函数一份直接 Map 声明的规则，第二份以现有 E_SCHEMA_DEF_MISMATCH 拒绝，不叠加多个局部证明或字段特例。core 和语言测试存储 AST 的只读盘点没有发现直接多份声明，但这不是所有消费者或宏生成代码的清零证明。互相冲突、重复匹配和匿名函数内的多份声明均加入既有负例回归；嵌套函数各自的注解与两参数局部 hint 不计为外层声明。升级时将同一函数的字段合并到一份 Map。

此批次仅面向 0.31.0 的 main；不改 0.30.1 维护线，不提前删除旧形式。最终测试结果、精确提交、review、CI 和消费者证据记录在 PR。
