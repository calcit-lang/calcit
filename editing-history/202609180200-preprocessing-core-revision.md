# 预处理缓存绑定 core revision

Entry dependency closure 中的内置与宿主 definition 只记录稳定标识符，Calcit package version 也可能在同版本开发构建中保持不变。因此 `dynamic-methods` 诊断缓存与 `--check-only` 成功缓存都必须额外保存 embedded core Snapshot revision。

core revision 不同时明确以 `core-revision-changed` 冷启动。旧缓存缺少该字段时按空 revision 处理，因此只会安全失效，不会误命中。
