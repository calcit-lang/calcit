# 证明函数 mappend 的容器分支

`&fn:mappend` 是 Fn `.mappend` 的入口：先 `f x` 后 `g x`，按第一个结果的类型做 List concat、Map merge、Set union、String concat，其余交给结果自身的 `.mappend`。旧实现只判断 v1，第二个结果始终是未收窄的泛型 B；返回函数的参数也没有 A 的证据，因此普通检查通过、独立 proof 失败。

修复在返回函数体内用已有 `hint-fn` 声明参数 A，并让每个容器分支同时判断两侧类型。String 分支保持只判断 v1，因为 `&str:concat` 本就接受任意第二参数并格式化；两侧类型不一致的 List 经 `.mappend` 回到 `&list:concat` 报错，Map/Set 不一致改在方法查找处报错，仍是错误。

回退分支 `.mappend v1 v2` 在无约束的 B 上无法静态分派；运行时 `assert-traits` 不算分派证据，只有 `:where` trait 约束能证明，而 core 没有 Mappend trait，引入它牵涉 0.29 已删除的 Map/Set `.mappend`，属于 API 决定，见 #2332。因为回退结果无法证明为 B，返回合同保持裸 `Fn`，不以声明代替证据。

验证：定义 `:tests` 覆盖各容器组合、方法调用、Fn 结果回退、调用顺序与输入不变、类型不一致及回调失败，native/JS 通过，WASM/WASI 编译拒绝登记为 unsupported。非 core 副本的完整 strict 只剩回退分支的 `E_DYNAMIC_METHOD_DISPATCH`；参数注解写错、只判断单侧两种反例仍被拒绝，检查不改写 Snapshot。
