# WASM 保留普通推断已经建立的 proc 结果证据

## 问题与选择

固定 proc 摘要可以说明结果属于 Enum 家族，却不一定包含本次构造的名义身份、参数和附加实现。
WASM 的 `infer_wasm_static_type` 对无返回类型变量的摘要直接返回，导致已经完成正常推断的名义 Enum
在后端再次变成普通 `enum`。显式 trait 调用因此无法找到实际存在的实现。

本次在原有固定摘要分支复用 `infer_static_type_from_expr`，仅采纳能够通过原有方向关系证明符合摘要的结果；
没有结果或不能证明时保留原摘要 fallback，避免反向弱化已知类型。
已有泛型 proc 的实参证明及结果替换分支不变。不增加按方法名猜类型的规则、检查器、诊断或公开入口，
也不改变 f64 ABI、运行时布局、实际方法选择和函数调用方式。

## 语义验证

在原 trait fixture 的 definition `:tests` 添加两个用例，保留原函数体、schema 和全部旧断言：

- 直接构造带有 `MyBar` 实现的 Enum，通过显式 origin 调用实际文本方法。
- 将 Enum 构造结果绑定到局部变量，再分别调用同名 `.zap` 的 `MyZapA`、`MyZapB`，验证实现身份不混淆。

两项构造的 payload 都有真实输出；共享运行器同时比较 native、JS、WASM 和真实 WASI 的执行与输出，
避免省略或重复参数求值。相同新增用例在固定基线中 native/JS 通过，WASM/WASI 在查找实现时编译失败；
候选四后端均通过。测试使用原结构化事务、revision 守卫及完整 readback，不使用 native call 替代。
新增用例沿用 `impl-origin-portable` 标签进入现有 WASM CI 选择组，不增加命令或 CI job。

原 trait 回归、core 四后端和完整编译器回归继续验证；不新增后端排除或放宽类型关系。

## 保留边界

真正未知的值不能因此获得某个具体 impl，显式 trait origin 的身份检查仍由原解析器完成。
本次只保留构造结果已有的证据，不声称补齐开放 Enum 的普通方法或公开 `contains?` 分发。
额外隔离对照中，这两条开放 wrapper 的旧后端错误仍被保留；不能把显式 origin 正控通过写成它们已经修复。
