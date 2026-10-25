# 独立 impl 别名的内部 callable 身份

同一基础 Enum 可以通过不同的源码 impl 定义形成合法别名。匿名方法 lowering 原先只用基础类型和方法名生成内部符号，导致第二个实现被误判为生成代码冲突。真实消费者是 Alerts 的 Confirm／Modal；已发布的显式命名 callable 过渡保持有效。

本次复用已有源码 provenance 查询，让生成名携带 impl 的源码定义身份。定义名按 UTF-8 字节无损编码，避免大小写、标点或 Unicode 归一化引入新重名；所在 namespace 继续由原生成函数 namespace 隔离。不用运行时计数或函数分配 id，以保留确定性。trait-backed impl 的标签实际是 trait 名，必须通过源码对应的已求值 impl 查找 owner，不能把 trait 定义误认为具体实现的 owner。

公共 Enum／Struct、trait 身份和方法解析优先级不变。源码重名、同一生成身份的不同执行体、真正的方法歧义仍拒绝；impl、trait schema 和接收者源码的缓存失效边继续保留，没有来源时沿用原回退行为。不新增入口、改写规则、Dynamic 或 unsafe。

用户语义放在现有 trait fixture 的 definition `:tests`：两个相同 trait 的独立 impl 别名返回不同 Number；另一个 trait 的泛型回调原样返回 Number／Unit。通过现有统一运行器实际回放，不复制运行器。原 native／JS trait 测试和低层来源、缓存与冲突断言保留。

验证使用未修改 main 的失败回放与候选修复的对照，并开启改写后校验。WASM 执行两个共享用例；WASI 执行不含间接回调的别名用例。泛型回调在 WASI 仍由既有 `E_WASI_COMMAND_INDIRECT` 拒绝，不改变能力门禁或将其描述为支持；完整 fixture 的既有 core Enum strict 发现另行保留，本次另以隔离的同源定义与真实调用验证完整 strict。最终命令、结果和 CI 记录写在关联 PR。
