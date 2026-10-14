# 结构化开放输入的 callback 证据

Copycat 0.30.0 迁移在 Ref watcher 的 callable proof 上被阻断。独立 fixture 证明：
显式 Ref<Dynamic> 的 callback 即使忽略输入并返回 Unit 也失败，Number / Map Ref 对照通过。
prefix / indirect 参数预处理原来在 proof 后清除所有新增 Dynamic 绑定，未区分裸开放值
与已有 Ref 结构所提供的开放成员合同。

修正复用原绑定与 callback context：结构化前置参数的开放成员可以继续流入回调；
裸 Dynamic 仍不提供具体参数证据，回调输入需要按 Dynamic 使用。此标记只恢复
数据输入证据，不继承或擦除 feature 权限，不新增 API、诊断、fix 或类型节点。

验证放在 add-watch! 的 Calcit :tests 与原 rejection harness：注册、数值守卫、
new/old 顺序、移除后不再触发；同时拒绝 Number-only 回调和未校验的数值消费，
覆盖显式 callable proof 与普通 strict，确保 Snapshot 不变。

实际 Copycat 的 JS callback 在补回输入绑定后也通过 watcher proof，无需改写 feature
权限；只剩日志 nil / undefined 返回的 Unit 证明与依赖门槛。不冒充消费者在匹配发布版
上完成验收，也不据此承诺任意 callback feature 都兼容。

新增负例还暴露普通 builtin 路径只在 audit 时调用共享 argument proof：Number-only
回调在显式 proof 中拒绝，却在普通 strict 中通过。移除该调用点的 audit 外围限制，
仅让已有 builtin callback 合同按普通显式开放输入政策执行；其他 primitive 维持原政策，
不复制检查逻辑、不放宽失败断言。一次将所有 primitive 都接入普通 proof 的尝试暴露
不相关的 core 语法处理限制，已撤回该扩展；最终 608 core tests 与正反例均通过。
