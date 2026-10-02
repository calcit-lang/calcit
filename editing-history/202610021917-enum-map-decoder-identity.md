# runtime-map decoder 的枚举声明身份

业务消费者 Copycat 的 `decode-map-as` 会拒绝同一个 Enum 定义刚构造的值：`expected enum :DemoEnum, got :DemoEnum`。直接构造和经过 Cirru EDN/class-mapper 的值都会分配新的 Arc，不能以外层指针相等作为唯一的声明身份。

修复复用 DataShapeGraph 校验已经使用的 `same_nominal_definition`：指针相同仍可直接通过，否则必须同时满足完整 definition reference 和数据 schema 相同。仅名称相同、不同 namespace、同一 reference 但 schema 改变，以及没有稳定声明 reference 的不同 Arc 均不因此获准。decoder 仍逐项校验 variant、arity 和 payload，输出继续使用目标 shape 的 canonical nominal。

用户可观察的回归放入现有 `test-runtime-map-decode`，并提供 definition-attached `:tests` 入口：正常直接构造、Cirru/class-mapper 往返、匿名枚举拒绝、错误 payload 拒绝。该函数本就在 native/JS 既有集成入口中调用，没有新增脚本。Rust 测试只验证无法通过普通表层构造精确控制的 Arc/声明 reference/schema 身份边界。

修改前已用已安装 CLI 运行附属测试，真实重现同名身份错误。修复后二进制的附属测试 1/1、原生 `calcit/test.cirru` 集成、Rust library 测试 924 项（另有一项忽略）、`cargo fmt --check` 和 `cargo clippy -- -D warnings` 均通过；Yarn immutable/compile 与 JS 中的同一 runtime-map 回归也通过。Copycat 实际 Recollect 模块的 `change-op` 列表，直接构造与 Cirru/class-mapper 往返均通过内存 eval，没有启动应用服务或读取持久化数据。

完整 `cargo test` 构建曾因磁盘空间不足失败，不能以 library 结果替代；全量 `yarn check-all` 尚未完成。Agent-interface 脚本在 `strict project workflow manifest` 场景遇到 compiler proof review 错误，已安装的未修改 0.28.0-alpha.3 CLI 同样失败；尚未据此证明精确 base SHA 的门禁状态，不调整预期或关闭门禁。仅清理本任务生成的增量编译缓存后继续验证，没有删除其它项目数据。应用仍使用正式 0.27.0，不改依赖为未发布的 core hash/alpha。
