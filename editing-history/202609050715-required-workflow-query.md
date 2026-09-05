# 2026-09-05 07:15 UTC — Required workflow query coverage / required workflow 查询覆盖

## 中文

- 将按 `VERIFIED_SHA` 查询 workflow runs 的示例上限从 20 提高到 100，避免活跃仓库中的重跑、手动运行或多个 workflow 挤出 required runs。
- 保留逐个确认 required workflow 的要求；列表中“没有看到失败”不能代替完整性检查。

## English

- Raise the example workflow-run query limit for `VERIFIED_SHA` from 20 to 100 so reruns, manual runs, and multiple workflows do not crowd required runs out of the result.
- Keep the requirement to account for every required workflow; merely seeing no failure in a partial list is not sufficient.
