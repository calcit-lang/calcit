# struct-match 的守卫与类型作用域

真实消费者从 Dynamic 读取值后调用公开 struct-match，运行时已有 struct? assertion，但匹配调用位于 assertion 返回之后；完整 strict 因而看不到内部匹配参数的 Struct 证据。

先试验了语句后保留成功分支事实。局部赋值、catch 恢复和闭包调用的反例说明，单看正常结束状态或统计预处理写入并不等于完整异常路径分析；该实验未提交，Rust 修改已撤回。

本次保留现有分支推断，把匹配代码放进守卫的成功分支。内部宏 &assert-then 复用原 assert 的消息验证、单次谓词求值、stderr 和 raise；公开 assert 仍只返回 Unit，参数及兼容顺序不变。struct-match 仍只求值输入一次，成功返回原对象，不复制、不解码，也不把事实泄漏到作用域外。

用户语义放在原定义的 :tests：名义身份与字段、错误文本、单次求值、匹配体异常和原 assert 合同。完整 strict 的真实名义返回与 catch 后不能证明开放值，加入既有 check-known-assertion 回归，不新增检查入口。跨后端按共享运行器实跑并如实记录既有 unsupported 边界。

通用 assertion 后的流敏感收窄仍属于后续类型检查工作；本次不宣布它已完成，不引入写入计数或新的 proof 状态机，也不改变 0.30.1 发布范围。
