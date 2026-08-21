# RFC：Calcit 生态类型质量门禁与 CI 采用方案

状态：Draft

日期：2026-08-21

## 摘要

Calcit 已提供 `analyze check-types`、`analyze weak-types` 和原生 `analyze quality`，并支持按
definition 保存 baseline。现在缺少的不是另一套统计脚本，而是一份跨模块的采用协议：什么是
定位报告，什么是发布门禁；新项目与存量项目如何采用；baseline 如何审阅和收紧；静态通过后
还需要哪些后端运行证据。

本 RFC 规定：

1. 类型质量门禁统一使用 `cr analyze quality`，不得在各仓库自行拼 JSON 或补 JS 比较脚本；
2. 新模块默认零容忍，存量模块提交按 definition 的 baseline 并只允许逐步收紧；
3. 类型门禁只是 CI 的静态层，不能替代 native/JS/browser/dylib 的实际运行测试；
4. 文档、模板、机器输出和 baseline 更新流程形成一条可追溯的生态规范。

## 已有能力

当前仓库已经具备：

- `check-types`：统计 full/partial/none 类型覆盖；
- `weak-types`：定位 schema/code 中的 Dynamic、nil/Optional 和其他弱类型证据；
- intent 分类：区分 unresolved debt 与已知 FFI/framework boundary；
- definition/path、impact、suggestion 等结构化定位信息；
- `analyze quality`：以非零退出码阻止类型质量回归；
- `--write-baseline` / `--baseline`：按 definition 保存预算，避免一处清债抵消另一处新增；
- 单一 JSON envelope，适合 CI 保存和消费。

概念指南见 `docs/type-guidance.md`，完整类库验收矩阵见
`docs/run/library-quality.md`。本 RFC 不复制这两份 how-to，而是为整个生态规定采用和演进政策。

## 问题

### 工具存在，但采用方式不统一

生态项目目前混合使用以下策略：

- 只运行默认 entry；
- 只执行 `--check-only`；
- 分别调用 `check-types` / `weak-types`，但不根据退出码形成门禁；
- 在 workflow 中用 shell、jq 或 JavaScript 自己比较统计数字；
- 每次 CI 重新生成 baseline，等同于自动接受新债务；
- 只生成 JS，不实际执行 Node/browser 测试。

这些 workflow 表面都叫“type check”，实际保证不同，后续升级无法判断失败是新增债务、输出协议
变化还是自定义脚本失效。

### 类型覆盖率不是正确性证明

类型 full 只说明声明和静态推断足够完整。它不能证明：

- JavaScript import/export 在目标 runtime 中存在；
- `unsafe-coerce` 的宿主值真的满足声明；
- browser binding 没有在 Node entry 使用；
- callback 参数、返回值和 exception 行为符合 FFI 契约；
- native dylib 的 ABI 与 `cirru_edn` 版本一致；
- 已生成 JS 与当前 `@calcit/procs` runtime 相容。

因此静态质量与后端契约测试必须分别保留，不能用一项绿色 check 代表全部质量。

## 统一质量层级

每个模块在 README 或维护文档中声明当前采用层级：

| 层级 | 必需证据 | 适用范围 |
| --- | --- | --- |
| Q0 Snapshot | `edit format` 无 diff、`--check-only` | 所有项目 |
| Q1 Type ratchet | `analyze quality` 零容忍或已审阅 baseline | 所有维护中的模块 |
| Q2 Public API | 公开 API full 优先、examples/docs/test entry | 可复用类库 |
| Q3 Backend contract | 实际执行声明支持的 native/JS/browser/dylib 路径 | FFI、workflow、应用 |
| Q4 Consumer | 至少一个真实下游回归 | 核心库、编译器、关键基础模块 |

层级是累积的。Q3 不能跳过 Q1，Q1 也不能声称已覆盖运行时正确性。历史项目可以从 Q0/Q1
开始，不因一次迁移被迫达到 Q4。

## 规范 CI

### 安装与版本

工具版本来自 `deps.cirru`：

```yaml
- uses: actions/checkout@v4
- uses: calcit-lang/setup-cr@0.0.9
```

普通项目不在 workflow 重复填写 `version`。setup-cr 的详细契约由
`08-21-setup-cr-version-and-toolchain-contract-rfc.md` 规定。

### Q0 + Q1 基础门禁

新项目：

```bash
caps --ci
cr calcit.cirru edit format
git diff --exit-code -- calcit.cirru
cr calcit.cirru --check-only
cr calcit.cirru analyze quality --format json
```

存量项目只把最后一行改为：

```bash
cr calcit.cirru analyze quality \
  --baseline config/calcit-quality.json \
  --format json
```

`check-types` 与 `weak-types` 用于本地定位和 PR 解释，不再承担自定义退出逻辑：

```bash
cr calcit.cirru analyze check-types --summary-only
cr calcit.cirru analyze weak-types \
  --only schema-dynamic,code-dynamic \
  --intent unresolved
```

CI 可以保存 JSON 报告，但不得解析若干子报告后自行发明总分。需要新的质量维度时，先扩展
`analyze quality` 的版本化协议，再由所有项目统一升级。

### Q2 公开 API

可复用类库至少增加：

```bash
cr calcit.cirru analyze check-examples --ns package.api
cr calcit.cirru docs format-md README.md --check
cr calcit.cirru docs check-md README.md --failures-only
cr calcit.cirru --entry test
```

没有 examples 且命令退出零不代表覆盖完成。公开 definition 必须由 runnable example 或明确的
测试 entry 覆盖。公开 API 新增 `Dynamic` 时，即使 baseline 仍有余额，也要求 PR 说明它属于哪种
边界、为什么不能使用泛型/trait/Enum/Option/Result。

### Q3/Q4 后端与消费者

- JS 模块：codegen 后实际运行 Node test；browser API 运行 headless browser smoke/contract test；
- native dylib：build、复制实际 artifact、由目标 `cr` 进程装载并调用；
- 多 entry：逐一运行声明支持的 mode/target，不能只测 default；
- 核心/基础库：记录至少一个真实消费者仓库、commit、entry 和命令。

后端矩阵由项目显式维护，不能交给 setup-cr 自动猜测。

## Baseline 治理

### 首次创建

```bash
cr calcit.cirru analyze quality \
  --write-baseline config/calcit-quality.json
```

生成后必须人工审阅：

- debt 是否确实属于当前项目，而非未加载依赖或错误 entry；
- intentional FFI/framework 分类是否准确；
- public API 的 Dynamic 是否可以先收窄；
- baseline 中每个 definition 是否有负责人或迁移方向。

审阅完成后将文件提交。CI 永远不能先 `--write-baseline` 再 `--baseline`。

### 更新规则

1. 一般 PR 只允许删除或降低 definition 预算；
2. 新增债务需要独立说明，不与无关功能提交混在一起；
3. 重命名/移动 definition 时，由工具提供可审阅迁移，不能整体重建以抹去历史；
4. 编译器新增质量维度时，先输出 migration summary，再更新 baseline schema version；
5. baseline 文件包含生成它的 quality schema version，不绑定机器绝对路径或时间戳；
6. intentional boundary 也进入可见报告，只是不与 unresolved debt 使用同一失败策略。

长期目标是让重点模块 baseline 归零，而不是永久维护一个“允许 Dynamic 的白名单”。

## 文档体系

避免同一命令在多个位置出现互相矛盾的解释：

| 文档 | 职责 |
| --- | --- |
| `docs/type-guidance.md` | 如何选择具体类型、泛型、trait、Option/Result |
| `docs/run/library-quality.md` | 可复制的本地与 CI 验收步骤 |
| 本 RFC | 生态采用、层级、baseline 治理与演进政策 |
| setup-cr README | 只说明工具安装和版本来源 |
| 模块 README | 声明本项目达到的质量层级与特有后端矩阵 |

新增质量能力时先更新机器命令和这张职责表，再更新模板；不把临时 shell/JS 脚本复制到多个
仓库成为事实标准。

## 生态推进顺序

### 第一批：参考项目

- `js-ffi`、`calcit-wss`：保持高类型覆盖，补 Q3 runtime contract；
- `calcit-http`：为 native callback/options 建立显式 FFI 契约；
- `bisection-key`：清理容器与泛型 Dynamic，验证存量 baseline 收紧流程；
- `calcit.std`：作为零容忍或低 baseline 的类库模板。

### 第二批：框架

- `memof`、`recollect`、`respo-calcit-workflow`：先收敛 store、dispatch、callback schema；
- 把确实属于框架开放边界的 Dynamic 标明 intent；
- 避免每个应用重复声明同一套框架生命周期类型。

### 第三批：大型应用

- Editor 和网站项目先建立 baseline；
- 以 namespace/definition 为单位收紧，不做一次性全量重写；
- 优先处理全局 state、组件 props、effect callback 和跨模块公共 API。

## 机器协议要求

`analyze quality --format json` 应保持：

- stdout 是单一 envelope；
- stderr 只输出人类摘要和进度；
- schema version 显式存在；
- failure reason 区分当前债务、相对 baseline 的新增、baseline 过期和工具配置错误；
- occurrence 包含 definition、namespace、path、kind、intent、impact、suggestion；
- 退出码稳定区分 quality failure 与命令/解析失败。

如果 GitHub annotations 或 SARIF 有明确需求，应由 `cr` 原生输出或提供统一转换器。各项目不再
自行实现一份脆弱的 JS parser。

## 实施阶段

### Phase 0：采用与模板

- 将本 RFC、type guidance、library quality guide 互相链接；
- 更新模块模板为 setup-cr 无 version + `analyze quality`；
- 选取四个第一批项目记录 baseline/zero-tolerance 实践。

### Phase 1：协议稳定

- 固定 quality JSON 与 baseline schema version；
- 补 baseline rename/move 和过期诊断；
- 提供统一的 GitHub annotation/SARIF 输出时仍保持原生命令为事实来源。

### Phase 2：后端契约接入

- 将 JS FFI unsafe/coercion 和 native ABI 风险纳入质量维度；
- quality summary 链接对应 Q3 测试，但不伪装成已执行 runtime test；
- 重点模块逐步从 Q1 推进到 Q3/Q4。

## 验收标准

1. 新 Calcit module 不需要自写 JS 脚本即可阻止类型债务回归。
2. 存量 module 的 baseline 不会因每次 CI 重建而自动放宽。
3. CI 日志能明确区分 Snapshot、type quality、backend runtime 和 consumer regression。
4. 至少四个参考项目采用同一套 Q0/Q1 命令，并记录各自 Q3 矩阵。
5. 新增 quality 指标通过版本化协议进入生态，不要求所有仓库同时修改自定义 parser。
6. `full` 类型覆盖不再被文档描述为 FFI/runtime 正确性的充分证明。

## 非目标

- 用单一分数比较不同规模项目；
- 要求所有历史应用立即零债务；
- 把 intentional FFI boundary 隐藏出报告；
- 让 setup-cr 隐式执行项目测试；
- 以静态门禁替代真实后端运行。

## 相关资料

- `docs/type-guidance.md`
- `docs/run/library-quality.md`
- `RFCs/07-26-static-semantic-analysis-rfc.md`
- `RFCs/08-21-setup-cr-version-and-toolchain-contract-rfc.md`
- `RFCs/08-21-js-ffi-runtime-contract-validation-rfc.md`

