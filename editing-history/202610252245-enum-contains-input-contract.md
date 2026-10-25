# Enum contains? 的内部输入合同与未完成的结果证明

完整 trait fixture 的 strict workflow 暴露 `&core-contains-enum-impl` 的匿名函数
没有参数合同：函数体调用 `&enum:count x`，却不能独立证明 `x` 是 Enum。
这里补齐 `Fn(Enum, K) -> Bool` 及泛型声明，保留原函数体。
`K` 继续开放，原 Number guard、索引范围、分数索引行为和错误文本不变；不能把键
直接标成 Number 来消除原动态错误边界。

原 trait fixture 的 `:tests` 验证自定义方法返回非 Bool 时仍被公共 dispatcher 拒绝，
开放 String 键和 nil 仍报错，接收者与键的异常仍按原顺序传播，不重复执行。
这两项使用原 native／JS 运行器的 ref／try 能力，不宣称增加 WASM 功能。
既有 strict-default 脚本复用 callable proof 单独检查此真实内部定义，防止仅凭运行结果
掩盖输入合同缺失；没有新增 checker 或统计门禁。

这只是公开 Enum `contains?` 修复的必要输入合同，不是完整后端修复。
原七项公开调用在最新 main 上 native／JS 各 7/7、WASM／真实 WASI 各 0/7。
曾尝试把特化局部证据写入仅供推断的方法表达式视图，再复用既有方法推断；七项
正例与九项扩展正例均四后端通过，但补充的自定义 Bool 方法反例证明：WASM 仍可能
执行内建索引分支而不调用实际方法。只修结果推断会掩盖这个选择／执行不一致。
该候选后端改动已撤回，临时副本保留反例，不把错误实现的通过作为完成依据。

后续必须把实际方法选择、输入证明和 lowering 对齐，再重新验证共享正例、自定义
Bool／非 Bool 方法、歧义、开放键以及副作用和错误。不能按方法名断言返回 Bool。
本次不改变公开 API、ABI、Dynamic／unsafe 或后端排除，不删除原测试，不发版。
完整 trait strict 越过 Enum 合同后还会到达既有 Struct helper 的未注解参数，仍未通过。
