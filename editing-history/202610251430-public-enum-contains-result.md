# 公开 contains? 对 Enum 保留选中方法的 Bool 合同

#2339：已知 Enum 接收者调用公开 `contains?` 时，native/JS 通过，WASM/WASI 陷阱。公开函数
先分派到接收者的 `.contains?`，再用 `bool?` 检查结果；在擦除到 f64 的标量后端里，未知来源的
0/1 不能被识别成 Bool，于是走到 raise。

修复沿用 #2365 对 `empty?` 的做法，不按方法名假定结果类型：

- Enum 的 `.contains?` 实现从 impl 里的匿名函数提成 core 内部定义 `&enum:contains?`，合同为
  `Fn(Enum, K) -> Bool`。键保持开放的 `K`，函数体仍先检查 `number? k`，非 Number 键保留原错误
  `Enum .contains? expected a Number index`，没有扩大 Dynamic。
- 预处理时，只有接收者已证明是 Enum 或 Struct（公开 `contains?` 中 List/String/Map/Set
  分支之外、实际走到 `.contains?` 的那一类），且实际选中的实现能独立证明参数并返回 Bool，
  才把公开调用改写成对该实现的直接调用。缺失、歧义或未证明时保留原 dispatcher 和运行时
  Bool 检查。匿名 `'Enum` 接收者也能命中，因为实现现在是有名字的定义。

验证：`contains?` 新增 core `:tests`，覆盖 Option 名义接收者与 `'Enum` 形参两种写法，
在 native/JS/WASM 上通过；原 #2339 的七个断言（隔离项目中的三个测试）在修复前 WASM 0/3、修复后 3/3。
