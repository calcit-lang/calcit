# 引用中 callback 的泛型推断与空集合分支证明

## 来源

Respo dispatch 对照中，`Controller<Op> { dispatch: Ref<Fn(Op) -> Unit> }`
经工厂函数构造可以通过，直接构造却退化成 `Ref<Fn(Dynamic) -> Unit>` 并误拒。
PR 审阅同时发现泛型空集合在 `if` 外层被提前拒绝，无法进入逐分支证明。

## 决策

- 沿已有的匹配 Ref 层取得 callback 输入证据，仅推断本次调用拥有的泛型；
  随后仍以原始类型完成双向 Ref 证明，不改变可变引用不变性与函数方差。
- `if`、词法绑定和 match 先检查各自出口，再在实际值上限制泛型构造器证明。
  空集合不绑定成员类型；非空、开放输入不能据此实例化外层泛型。
- 新增负例暴露出 `(deref callback-ref)` 直接作为调用头时漏检参数。
  将局部函数参数检查抽为共享入口，计算得到的函数头复用相同类型与泛型关系检查。
  不新增动态 fallback，也不对 Respo 名称做特判。
- 间接调用检查覆盖了 macro 生成的 loop 初始调用；同类空集合仍是合法 nominal
  集合初始值。nominal 边界复用既有空构造器判定，避免把它误认为开放输入；增加
  空 `List<Item>` callback 实参的可执行正例。
- 审阅提出移除 pairs-map 的第二个 list? 分支。普通求值可通过，但严格审计证明
  删除后 &list:first 接收到未收窄的 P，报告 E_CALL_ARGUMENT_UNPROVEN。
  因而保留现有分支，不借本次修复扩展 assert 的控制流收窄语义。

## 验证与边界

成功合同存入 definition `:tests`，通过共享 native/JS 运行器验证实际 callback
调用与状态更新；现有 known-assertion harness 验证非空/开放分支、错误 callback
实参和错误引用写入，并检查拒绝后没有应用产物。

继续回放 Respo 的冻结 65 场景，不以局部通过代替完整 framework strict、旧 Tag / cursor-list
兼容或生产 slot / generic 路线决策。具体提交、执行结果和未完成项记录在 PR 与关联 issue。
