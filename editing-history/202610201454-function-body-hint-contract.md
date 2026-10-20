# 函数体注解与定义合同使用同一套证据

## 问题

CodeEntry 的 Fn 与函数体 Map 描述同一个函数。此前 preprocess 优先整份使用 body hint，导致 return-only 注解清掉原参数、泛型与 trait 证据；反过来，body hint 也能把公开 Number 当作 String，或自行声明调用方没有承诺的 ToString 能力。普通与 strict 一起通过并不能证明这些调用安全：实际 native/JS 会失败。

另一个同类反例是固定第二参数被注解为 rest：单纯 Fn 子类型关系能接受公开的两个实参，但函数体把实际 String 当作 List，JS 的 nth 随后失败。

## 设计

保留字段是否实际出现的信号。只为缺省字段继承 CodeEntry 的已声明值；显式 Dynamic 不是缺省。feature 合集与既有 async 调用语义不变。

在这些声明用于 body 参数推导之前，复用 caller-rigid CallTypeProof 校验完整 Fn 关系。trait 能力只从原公开合同进入证明，不能从待验证的 body 约束自行得到。显式参数与 rest 布局复用既有 definition/schema 形状校验，不增加 checker、诊断类别、函数名规则、unsafe 或 Dynamic 放宽。

兼容的两参数 hint 仍保留，其未知签名问题及 CodeEntry 泛型全局身份工作不在本批次宣称完成。隔离对照确认不同名字泛型的旧 strict 失败在基线已存在，不以本改动关闭那些任务。

## 验证与边界

合法的 return-only bounded 参数与 rest 参数写在现有 trait-bound fixture 的 definition :tests，并复用 native/JS 共享运行器。三个非法声明复用 known-assertion harness，覆盖普通、独立返回 proof、完整 strict、native 和 JS emit，并验证源码未改。

原 public-return 负例保留原源码及拒绝要求；新校验更早报告 E_SCHEMA_DEF_MISMATCH，因此精确检查新的合同诊断、源位置与源码不变，不把任何编译失败当成通过。

此批次仅面向 0.31.0 的 main；不改 0.30.1 维护线，不提前删除旧形式。最终测试结果、精确提交、review、CI 和消费者证据记录在 PR。
