# 核心转换与副作用边界契约基线

为 #1568 建立首批可 review 的 Cirru EDN 发布证据，复用现有查询，不新增运行时 API registry 或公开命令。保留 schema AST 与泛型约束、名义声明及 optional/rest arity，方法要求 proven 派发并追溯声明；实现路径不作为公开名字冻结。

范围限于已审阅的转换、解析、文件读取、时钟、Ref watch 和 Option/Result/FsPath/Ffi 宿主边界。部分 candidate 不冒充全量冻结，不关闭 milestone。失败记录不是自动语义证明，backend 覆盖和别名排期继续由原 issue 收尾。现阶段拒绝所有未显式迁移的 breaking change，集中迁移例外留待同一 issue 完成。

维护脚本检查当前与历史基线，防止只更新基线绕过签名/失败契约变化；负例涵盖删除、参数/返回、泛型约束、optional arity、receiver、方法 schema 和 feature，正例涵盖新增与内部实现重构。语义仍复用 Calcit attached tests，不引入新分析器。

验证：同步到已合并的枚举推断修复后，fmt、clippy、完整 cargo test、yarn check-all（含 Agent interface）通过。工具边界测试包含真实临时 Git 仓库的 PR merge-base/push 父提交防绕过检查；原生 EDN 完整往返保留类型 quote 与 symbol。全局正式 Calcit 在已迁移的 Respo 临时消费者严格检查通过，未修改真实业务项目。
