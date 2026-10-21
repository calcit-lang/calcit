# 泛型调用在证明前汇总输入证据

## 问题与决策

真实消费者发现 `merge` 把后续 `Map<Tag,Dynamic>` 错推为首个默认 Map 的 Bool payload。
独立复现进一步显示：普通检查与完整 strict 可共同接受 Bool 返回声明，而 native/JS 返回 Number 12；
普通泛型函数也可复现，因此不能增加 merge 专属规则或恢复弃用入口。

现有 CallTypeProof 在一次调用开始时，从每个实际参数独立收集 callee-owned 变量的证据。
共同上界必须来自实际注解，并通过保持 caller 变量 rigid 的方向证明接纳全部贡献者。
显式开放 producer 可保留 Dynamic；封闭但冲突的输入不能凭空得到 Dynamic，冲突变量也不能作为
精确返回证据泄露。不改变逐参数 speculative proof 的事务与回滚语义。

函数、方法、内部 primitive、rest/spread 与泛型 callback 的实际调用复用此准备步骤。
方法调用不再把仅用于查询展示的 receiver-specialized 合同当作全调用证明。
方法的 primitive 优先读取已有公开 schema，而非宽泛 runtime 签名；同一次调用的准备与检查复用
同一份解析后实参证据，避免重复解析名义 trait 定义产生不同身份，也不把未解析输入当作显式 Dynamic。
现有空容器、Option none、Result err 的无 payload 规则在准备证据时复用；独立的已知投影仍可保留。
不增加检查入口、诊断编号、固定改写、Dynamic/unsafe 合同或统计门禁。

## 兼容与验证

合法开放值仍能传递、包装和合并，但不能因为实参顺序而假收窄到具体返回类型。
保留 Ref 不变性、函数输入逆变、调用方词法泛型身份及 where 约束；合并运行值和求值顺序不变。

CLI 拒绝与 type-at 证据扩展现有 fix_cli 矩阵；内部 binder 的不变条件扩展原 CallTypeProof 单元测试。
保留原 post-lowering 的 caller-generic 方法拒绝断言，并改善失败输出，便于核对原方法合同没有被宽泛签名替换。
用户语义扩展 core merge 原 :tests，保留原覆盖顺序断言，增加开放 Map 的前后顺序、方法与 postfix、
Some(nil) 和 Bool 字段保留。复用原 native/JS/WASM 运行器，不新增 runner 或排除项；原混合值测试的
WASM recursive-specialization 限制仍存在，支持的方法覆盖顺序测试继续实际运行。
提交与 PR 记录最终执行结果及实际后端范围；本修复不表示普通方向检查总单或整个版本已经完成。
