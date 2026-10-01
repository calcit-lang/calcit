# 补齐标量转换与显示角色的候选契约

关联 #1456 / #1458。已推荐的 String、Number、Bool、Tag、Symbol `.to-string` 在现有 query 中均为 proven，零个显式参数，返回 String。此前候选基线只记录 ToString trait、具名 to-string 与 FsPath 方法，缺少这些实际消费者常用的标量派发契约。新增五条实际调用证据，同时纳入 Debug/Show 公开 trait 声明，保持三种角色分离；不冻结内部 helper 路径，也不制造第二份运行时 API registry。

开放 JSON/Cirru EDN 解析返回 Result<Dynamic,String>，目前 query 方法证据仍为 open。这是有明确边界的开放数据，不把它为了增加基线覆盖而改成 proven 或杜撰具体 payload。基线仍是部分 candidate，完整 backend 矩阵、兼容退场及迁移例外由原 milestone 跟进。

新增维护工具测试检查这五种接收者的原生 call-type 语法，证明将返回值变宽到 Dynamic 或把 Debug 方法替换成 to-string 会触发既有契约拒绝。通过 Calcit CLI 为 `to-string` 增加附带测试，使用五种实际 `.to-string` 方法调用检查结果和 String 类型，便于直接 review，并由既有 native/JS 共享路径执行；不重复增加 Rust 语义测试。该测试不声称所有标量在 WASM 的转换都已受支持。

std 的查询读 API 已由实际 0.2.36 tag 发布并用真实 native 文件接口验证，因此把过期的“待发布”文档改成明确的 alpha.3 配套说明。不将预发布组合写成稳定版完成，也不提前关闭 milestone。
