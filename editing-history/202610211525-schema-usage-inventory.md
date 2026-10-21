# 同一次 schema 分析复用源码引用清单

Respo 的完整 strict 在正式 alpha.1 配对下运行约九分钟。进程采样将主要工作定位到 schema 候选收集：每个候选都会重新编译和追踪所有项目调用方，然后才筛选当前目标。普通框架检查和业务测试并没有同等开销，因此不应通过删掉 strict、跳过定义或放宽类型解决。

在一次不可变 Snapshot/entry 的 schema evidence 收集中，按调用方 namespace/definition 保存普通源码引用追踪的结果或失败消息。第二个候选复用清单，但仍按自身目标过滤引用，再进入原来的 assertion-proof checkpoint 逐项重新编译调用方并证明参数类型。缓存不包含参数推断结果、独立证明结果或已编译函数，也不跨命令、Snapshot、entry 或 source 改写保存。

单项 schema synthesis 使用新的空清单，现有严格/宽容错误处理分支保持不变。未增加命令、flag、诊断编号、源码改写规则或计数门禁；既有 incremental evidence bypass 保持原状。宏来源、源码坐标、不可用调用方和失败诊断仍在同一条路径处理。

验证扩展现有 CLI 集成测试，而不是增加语言测试或平行检查脚本：此改动不改变 Calcit 运行时语义，真正需检验的是 bulk evidence 与单项新分析的候选和 origin chain 相同，以及复用清单后仍拒绝不成立的调用方证明。现有用例覆盖 exact、usage-derived、冲突、宏边界、错误断言、回调、样例 namespace、Ref 和部分 schema；原断言保留。相关 14 项 schema CLI 回归通过。

真实消费者验证固定 Respo 的同一提交、原 Snapshot 字节和同一正式 runtime，分别用同一 dev 构建配置的优化前后 CLI 执行完整 strict，保留报告并比较候选/诊断。正式 alpha.1 约 544 秒的历史数据与此处 dev 对照分开记录，不混用构建配置计算加速比。CLI 候选只能作为未发布实现的验证，不能替代消费者正式配对发版验收；结果与 PR/Issue 门槛以当时实际运行记录为准。
