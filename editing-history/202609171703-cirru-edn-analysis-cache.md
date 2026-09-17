# 增量分析缓存改用 Cirru EDN

## 背景

Calcit 体系默认以 Cirru EDN 表达结构化数据，JSON 只作为显式互操作格式。增量分析缓存属于 Calcit 自己维护的本地数据，
不应以 JSON 文件作为默认协议。

## 修改

- definition cache 从 `.calcit/analysis-cache-v1.json` 改为 `.calcit/analysis-cache-v1.cirru`。
- 新增的 merged input cache 使用 `.calcit/analysis-input-cache-v1.cirru`。
- 两份缓存统一通过 `cirru_edn::to_edn`、`format`、`parse` 与 `from_edn` 编解码。
- 更新 CLI 元数据、测试、agent 文档、静态分析文档与原有 editing history 中的最终文件名。

## 兼容边界

旧 JSON cache 不迁移也不读取；它们只是可重新生成的本地缓存。首次运行新版 `--incremental` 会按缺失缓存执行冷扫描，
随后写入 Cirru EDN cache。
