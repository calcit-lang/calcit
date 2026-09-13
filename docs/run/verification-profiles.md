---
title: "Calcit 声明式验证 Profile"
summary: "用 calcit analyze verify 在一次共享项目加载中按确定顺序验证多个 entry"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "verification profile"
  - "analyze verify"
  - "发布验证"
entry_for:
  - "calcit analyze verify"
  - "verification profiles"
id: core/run/verification-profiles
parent: core/run
related:
  - core/run/workflow-entrypoints
  - core/run/library-quality
  - core/run/entries
requires:
  - core/agent
---

# Calcit 声明式验证 Profile

`calcit analyze verify` 把一组 Calcit 自己拥有的只读检查声明在 Snapshot 中，并在一次命令中按固定顺序覆盖多个 named entry：

```bash
calcit calcit.cirru analyze verify --profile release
calcit calcit.cirru analyze verify --profile release --format json
```

它属于 `analyze`，不会增加顶层 `calcit verify`。Profile 只编排现有严格预处理与 analyzer 事实，不运行 shell、部署或外部构建，也不修改 Snapshot。需要改写源码时仍使用 `calcit fix` 的 preview/apply 闭环。

## Schema v1

配置放在 Snapshot 顶层 `:verification`：

```cirru-edn
:verification $ {} (:schema-version 1)
  :profiles $ {} $ :release
    {} (:on-failure :stop)
      :entries $ [] :default :browser
      :checks $ [] :strict :dynamic-methods :quality
```

- `:schema-version`：目前必须为 `1`；缺失或未知版本会在任何检查运行前失败。
- `:profiles`：profile 名称到配置的 map。
- `:entries`：按声明顺序执行的 named entry；每项必须存在于顶层 `:entries`，不能为空或重复。
- `:checks`：按声明顺序执行的检查；不能为空或重复。
- `:on-failure`：可省略，默认为 `:stop`；`:continue` 会继续收集后续检查结果，适合一次性修复多个失败。

v1 支持三个 check：

| Check | 复用的语义 | 通过条件 |
| --- | --- | --- |
| `:strict` | 与 `--check-only` 相同的 init/reload 严格预处理 | 没有 warning 或 error |
| `:dynamic-methods` | 与 `analyze dynamic-methods --max 0` 相同的动态 method 诊断 | finding 数为零 |
| `:quality` | 与无 baseline 的 `analyze quality` 相同的 zero-debt 只读分析 | 没有 quality violation |

同一 entry 同时选择 `:strict` 与 `:dynamic-methods` 时，两项共享一轮预处理及类型推导结果；不会为了生成第二份报告重新解析或重新推导。不同 entry 的 module 顶层加载结果也会按 module path 在本次命令内缓存。Entry 的 target 来自其 `:target`；未声明 target 时使用 `:mode` 作为结果中的目标标签。

Profile 不把 `test`、JS/WASM/WASI codegen、Markdown 文档执行或外部消费者回归伪装成静态检查。它们可能运行用户代码、写生成目录或需要外部 host，继续作为发布流水线中的显式步骤。后续若能复用只读 compiler phase，可扩展新的 schema 版本，不向 v1 静默加入语义。

## 确定性与失败行为

执行顺序固定为 `:entries` 外层、`:checks` 内层。`:stop` 在第一项失败后停止；`:continue` 运行全部组合。任一已运行检查失败时，命令以非零状态退出。

Human 输出是简洁 Markdown，按 entry/check 使用 heading 和列表。自动化应使用 `--format json`；stdout 只包含一个 envelope：

```json
{
  "schema_version": 1,
  "command": "analyze.verify",
  "revision": "md5:...",
  "data": {
    "profile": "release",
    "on_failure": "stop",
    "status": "passed",
    "checks": []
  },
  "diagnostics": []
}
```

每个已运行 check 都包含 `check`、`entry`、`target`、`revision`、`status` 和带稳定 `code` 的 diagnostics。Profile 不存在、schema 无效、entry 不存在或 check 未知时，在运行检查前返回 `E_VERIFY_CONFIG`；JSON stdout 仍是一个完整 envelope。

## 发布工作流边界

Profile 适合收敛重复的 Calcit-owned 静态门禁，但不能代替完整发布验收。推荐顺序是：

1. `calcit edit format` 后用 Git 检查 Snapshot 是否干净；
2. 运行 `calcit analyze verify --profile release --format json`；
3. 显式运行 definition `:tests`、Markdown 示例、目标 codegen/runtime 和真实消费者回归；
4. 只在精确提交的全部 CI 成功后发布。

这样 Agent 可用一个稳定 envelope 读取静态结果，同时人类仍能看清哪些步骤会执行代码、写生成产物或依赖外部环境。
