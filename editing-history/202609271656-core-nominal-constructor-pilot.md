# 2026-09-27 Option/Result 具名构造器迁移试点

## 决策依据

0.25.0 milestone 要求构造器、方法与内部函数各有稳定的推荐入口。String 的 `.contains?` 同时属于跨容器 `Contains` trait，单独改名会扩大范围。`%some/%none/%ok/%err` 的 core 定义则直接委托 `Option/Result` 具名 Enum 构造；旧升级文档已经把具名定义直接调用列为推荐写法，Respo、Quamolit 和 Timegrass 有真实 helper 调用。因此本试点选择 Option/Result 构造 helper，不同时改动 String API。

## 语义和兼容边界

推荐 `Option :some value` / `Option :none`、`Result :ok value` / `Result :err error`。四个旧 helper 在 0.25.0 标记弃用但仍可运行；最早在 0.26.0、且真实消费者有迁移证明后才考虑移除，不增加永久别名。直接构造与旧 helper 产生相同 nominal Enum 值，payload 在原调用位置求值一次；错误 payload 仍在构造前抛出。错误堆栈可能少一层旧 helper 框架，迁移不声称字节级栈文本不变。

`core-nominal-constructor-v1` 是现有 `calcit fix` 的显式规则，不加入已冻结的升级 preset。自动改写必须有编译器解析到 `calcit.core` 的源位置、正确 arity、不被局部类型名遮蔽，并且只穿过已核对按一次分支展开的 core `let`/`cond` 宏。函数值引用、未知宏、引号数据、定义附带的 examples/tests 与无法定位的生成源码留给人工 review；不插入 Dynamic、unsafe 或业务默认值。应用使用已有 revision、subtree fingerprint 与 staged preprocess 门禁。

## 验证方式

- Option/Result definition-attached `:tests` 直接断言名义值、接收者方法及 payload 失败传播。
- CLI fix 集成测试覆盖嵌套调用的单次改写、stale revision 拒绝、apply 后幂等、遮蔽和函数值 review。
- `calcit/test-helper-inference.cirru` 的同一 Calcit `:tests` 由既有检查脚本在 native、生成 JS 和 WASM 执行；弃用标记也使原 `%some` 示例触发零债务门禁，现改用直接构造，仍要求推断证据精确。
- 文档中两处旧 helper 参与 `Option` 整值相等比较的示例，在直接构造后暴露了泛型精度不同的告警；改为先 `match` 再比较具体 enum/trait 定义，不用 Dynamic 或旧 helper 掩盖边界。
- 仓库 WASI 示例在定义源码上预览并通过 staged 验证；Respo 的 `respo.app.task/normalize-task` 在隔离副本上预览、应用并再次预览，definition test 改写前后均通过。原 Respo 仍锁定 0.24.3，未修改其工作树。
- 按仓库门禁完成 docs check-md、Rust/JS 回归和 PR 最新 HEAD 检查；合并后核对 main 精确 SHA workflows。Respo 全项目 JS codegen 目前被 `respo.controller.resolve/build-deliver-event` 的旧 `target-listener` 类型告警阻断，不能把该项写成通过。
