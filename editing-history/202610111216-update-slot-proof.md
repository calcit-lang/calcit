# update 的开放实现与精确调用合同

## 下游阻塞

Timegrass 完整 strict 报告指出 core update 将开放容器取得的 Dynamic 值传给
Fn<T>(T)->T。该 T 没有与接收者的 key/value 关联，不能从回调声明借用输入证明。

既有 checked-call-contract 已为 Map/List 提取精确接收者、key/value 和回调合同；
已知 Struct 字段也有 specialization。本轮复用这些机制，将实现的 fallback 声明
改为 Fn(Dynamic)->Dynamic，并检查 List/Enum 的 Number 索引及命中的 Struct Tag key。
不增加 compiler special case，也不使用 assertion/unsafe 将开放 payload 标成 T。

## 行为与反例

定义附带测试覆盖 Map/List 现有槽、缺失 Map/Struct 槽跳过回调、原 Struct 不变、
Struct/Enum 更新、回调异常传播和开放容器的非法索引拒绝。精确 List 的开放索引
本来就会在预处理被 E_ERASED_GENERIC_RELATION 拒绝；运行时负例通过实际解析取得
开放容器，而不是弱化这条检查。

既有 check-known-assertion 加入四种精确调用的 native/JS 静态反例：Map/List 返回
String 替换 Number、List String 索引、Struct 字段回调返回错误类型。失败必须保持
Snapshot 不变，JS 不得留下应用产物。开放容器配合 inc 的反例必须由 callable
审计报告 argument 3 缺证据。第一次脚本误把预期 exit 1 当作成功命令调用，已改为
检查拒绝状态并解析其 JSON 诊断。

完整 core 定义测试通过 native 608/608、JS 607/607（既有 JS 排除 1 项）。新测试在
WASM 实际遇到局部 &struct-def:new 与 try 的不支持诊断，已按具体测试登记 shared
WASM/WASI 排除；原有 update collection parity 排除保持。不能将排除算成 WASM 通过。
默认 clippy、fmt 和既有 reviewed core API contract 检查通过；后者覆盖其既有 scope，
不表示此次 fallback schema 未变化。

## 下游后续

新 core 的 Timegrass 隔离 alpha.30 源码探针消除 update/each 的 core 诊断后，暴露
ACK 和 metrics 调用点。探针尝试将间接 swap!/update 写成显式状态回调，再从真实
Ref<SyncMetrics> 声明回调参数，并在 dirty-rev 比较前解码 Number。仍需核对完整
strict 和真实同步测试结果；未通过前不把这些应用探针当成正式验收。

全部报告和生成产物留在忽略目录。完整 Rust 回归已退出 0：25 个 suite、1664 passed、
1 ignored；扩展的 known-assertion 脚本也已通过，包括新增的精确回调错误及开放
callback 拒绝。分支未推送，正式下游工具链没有升级。
