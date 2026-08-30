# Calx benchmark review 修复

## 中文

处理 PR #535 的 benchmark correctness、instrumentation、portability 与文档 review：

- ordinary Calx compile API 恢复无计时路径；只有 measured API 创建和读取阶段 timer。
- one-shot pure execution 在计时前准备 owned VM 参数，使它与 hot execution 的计时边界一致。
- benchmark orchestrator 从 Cargo JSON artifact 读取实际 executable path，兼容自定义 target directory 与 Windows executable suffix，并明确报告 spawn error。
- 单 case runner 从编译时嵌入的 resolved Cargo.lock 读取 calx_vm 版本，避免与依赖升级漂移。
- 文档明确 JSON stdout 只保证成功路径；失败走 stderr/nonzero exit。fixture 数量、benchmark 已完成阶段和后续路线同步到当前状态。
- 修正 cached Calcit callable baseline 的英文歧义，并为新增 benchmark helpers 补充职责注释。
- manifest version 未升级：本次是功能 PR，不是 release；仓库版本同步规则在发布阶段执行。

由于 pure-execution 计时边界改变，原始 baseline 必须在本修复提交的干净工作区上重新采集，不能沿用旧 JSON。

---

## English

Address PR #535 review findings covering benchmark correctness, instrumentation, portability, and documentation:

- Ordinary Calx compile APIs use the untimed path again. Only measured APIs create and read stage timers.
- One-shot execution prepares its owned VM arguments before the pure-execution timer, matching the hot-execution boundary.
- The benchmark orchestrator reads the actual executable path from Cargo JSON artifacts, supporting custom target directories and Windows executable suffixes, and reports spawn errors explicitly.
- The single-case runner reads the calx_vm version from the resolved Cargo.lock embedded at compile time, preventing dependency-metadata drift.
- Documentation limits the JSON stdout guarantee to successful runs and describes stderr/nonzero failures. Fixture counts, completed benchmark work, and remaining roadmap items now match the implementation.
- Ambiguous cached-Calcit-baseline wording is clarified, and the new benchmark helpers have concise responsibility documentation.
- Manifest versions are not bumped because this is a feature PR rather than a release; the repository synchronizes versions during the release stage.

Because the pure-execution timing boundary changed, the raw baseline must be recollected from a clean commit containing these fixes rather than reusing the old JSON.
