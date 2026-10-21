# 通过已有 config unset 清理陈旧命名入口

关联 #2104。真实旧项目保留了指向已不存在测试模块的命名入口，完整 strict workflow 会在读取所有入口时阻塞；原 CLI 只能 unset target，Agent 无法在不手改 Snapshot 的情况下删除已确认无效的完整入口。

扩展现有 `config unset` 的 key 为 `entry`，复用 `--entry` 指定待删除入口，不新增顶层或平行命令。必须显式指定名称，default 永远不可删除，不存在的名称明确报错。只删除 Snapshot 中的一项配置，不操作源码、模块目录或业务文件；是否确实废弃仍由维护者核对，不自动选择或删除失败测试。

将已有 config unset 接入同一个 staged transaction 白名单。预览和提交继续复用 Snapshot loader、写锁、原子替换及 entry 粒度的 revision；入口变化后旧 guard 拒绝，独立入口修改可以保留，后续操作失败不会留下部分删除。不放宽语言类型检查，不伪造空入口，也不增加迁移规则或专用运行器。

验收放在现有 CLI/文件系统测试：保留原配置测试，覆盖显式名称/default/缺失保护、dry-run、冲突、事务回滚、存活入口和定义元数据保持，以及删除前被缺失模块阻断、删除后通过完整 strict。真实 TCO 公开源码的隔离副本只验证结构化元数据编辑及源码保持；本地候选尚未发布，该探针不等于正式工具链配对或业务运行验收。

扩展非空 metadata 回归时复现 #2272：相同字节的入口，12 次预览产生两个 scope guard；原 main 基线的既有 config set 事务也复现。既有指纹直接将 HashMap 迭代顺序写入 hash，因此不能只用空策略/slot 的样例宣称删除事务可用。

修复仍在同一个 json_fingerprint helper：先转换为 JSON value，再确定性排序所有 object key 后 hash；数组顺序不动，不改变存储的 Snapshot、scope 单元名或令牌格式。用确定性自定义 serializer 的正反对象顺序验证，而不是靠随机 HashMap 种子概率；真实 value 与数组顺序变化仍产生不同指纹。CLI 回归同时填充被删入口和存活入口的多策略及具体 Enum slot，检查跨进程预览、未改动提交和真实冲突。升级 CLI 后重建临时 guard，不兼容接受旧的非规范指纹。
