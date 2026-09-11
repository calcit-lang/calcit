---
title: "Calcit 类库项目验收与质量门禁"
summary: "面向 Calcit module/library 的发布前验收矩阵：快照、配置、类型、examples、文档、入口、消费者回归与 CI 门禁"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "library quality"
  - "module acceptance"
  - "类库验收"
  - "模块质量"
entry_for:
  - "validate Calcit library"
  - "Calcit library CI"
  - "Calcit 类库如何验收"
id: core/run/library-quality
related:
  - core/run/upgrade
  - core/run/entries
  - core/features/static-analysis
---

# Calcit 类库项目验收与质量门禁

这份清单面向会被其他 Calcit 项目加载的 module/library。类库质量不能只由默认入口“能运行”证明：公开定义可能由外部消费者调用，替代入口和文档示例也可能不在默认 call graph 内。

## 结论标准

一次可发布的类库验收至少应证明：

1. Snapshot 可解析、可规范化，且没有未审阅的旧配置迁移。
2. 每个 entry 的 mode、模块和 type-slot 绑定明确。
3. 默认严格预处理没有类型 warning/error；新增公开 API 的类型关系可由 schema 与推导证明。
4. 公开 API 的 examples、Markdown 示例和项目测试真实执行通过。
5. 所有声明支持的 native/JS entry 都完成运行或 codegen。
6. 至少一个真实消费者项目完成回归；结构性编辑先在 Snapshot 临时副本验证。
7. CI 执行与本地相同的门禁，且工作区在格式化后保持干净。

## 1. 快照与配置

开始修改前先读取当前 Agent 指南：

```bash
calcit docs agents --contract
```

检查并规范化目标 Snapshot：

```bash
calcit calcit.cirru edit format
calcit calcit.cirru config show
git diff --exit-code -- calcit.cirru
```

`edit format` 是可恢复的规范化步骤，不是完整 linter。它会继续完成格式化，同时对以下情况写入 stderr 告警并给出后续命令：

- `compact.cirru` 文件名会被拒绝；先确认它是有效源码并复制/重命名为 `calcit.cirru`。仅 `edit format` 可一次性迁移早期 direct quote 与顶层 `:configs`，并输出迁移计数；普通 loader 仍严格拒绝旧结构。
- `W_LEGACY_ANY`：schema 中的旧 `:any` 被规范化为 `'Dynamic`；其他旧 type tags 也会在已知类型位置改写为 quoted symbol。
- `W_DYNAMIC_TYPE_DEBT`：本地定义仍有 unresolved dynamic；format 不会猜测类型并自动改写语义。

CI 中运行 `edit format` 后必须检查 diff，否则“命令成功”只说明文件可规范化，不说明提交已经采用规范格式。告警不会让 format 失败；需要严格门禁时，继续执行下文的 JSON 静态分析并按项目策略判断。

配置验收至少检查：

- 使用 `:entries.default`，不再主动新增顶层 `:configs`。
- 每个 entry 有明确的 `:mode :native` 或 `:mode :js`。
- named entry 是完整配置，不继承 default 的 `:modules` 或 `:type-slots`。
- `:init-fn` / `:reload-fn` 使用定义 symbol；旧字符串会在规范化写入时迁移。
- 项目、依赖模块和脚本只使用 `calcit.cirru`；`compact.cirru` 必须先按升级手册迁移。

## 2. 静态类型质量

先看覆盖摘要，再定位未解决的动态类型：

```bash
calcit calcit.cirru analyze check-types --summary-only
calcit calcit.cirru analyze weak-types \
  --only schema-dynamic,unresolved-type-slot,code-dynamic \
  --intent unresolved \
  --summary-only
```

若摘要有命中，去掉 `--summary-only` 获取 definition、Snapshot path、detail 和 suggestion。大型类库可用 `--ns-prefix <package-prefix>` 缩小范围；默认报告已排除依赖/core，不要为验收本库而随意加 `--deps`。

推荐方案按语义选择：

- 输入和输出共享同一类型：声明 `:generics` type variable。
- 只要求某种能力：使用 trait 和 `:where`。
- list/map/set/ref：保留元素、键值或状态类型参数。
- 有限的异构值：使用 enum，而不是用 `:dynamic` 绕过检查。
- JS FFI、global state 或 macro 边界确实无法静态确定：保持边界窄，并显式声明 `:features $ #{} :js-ffi`；进入 typed code 前 validate/convert。

`check-types` 与 `weak-types` 是定位报告；它们不重复判断 Dynamic 能否进入 typed code，也不提供比例、shape/family 排名。新类库以默认严格预处理的 warning/error 为类型门禁，不创建 quality baseline。

已经提交 quality baseline 的存量类库可在 0.14.x 继续执行原命令，作为清债期间的兼容 ratchet：

```bash
calcit calcit.cirru analyze quality --baseline config/calcit-quality.cirru
```

后续只应降低已有 baseline；清零后删除命令和文件。不要为新项目生成 baseline，不要用 ignore warning 或批量 `:dynamic` 让数字看起来通过。0.15 将不再把 coverage/Dynamic 数量作为独立类型正确性策略；迁移后的 CI 直接依赖严格检查、公开 API 检查、目标后端测试与真实消费者回归。

将 baseline 保持在 Git 中，并声明为文本形式的生成文件，使 GitHub 语言统计忽略其行数。在项目根目录的 `.gitattributes` 添加：

```gitattributes
config/*-quality.cirru text linguist-generated=true
```

baseline 继续参与 CI、版本控制和文本 diff，但不计入 GitHub 语言统计。只有外部工具明确要求
JSON 时才把输出路径写成 `.json`；仓库内配置默认使用 Cirru EDN。任何 baseline 更新仍须按
definition 审阅，不能因其属于生成文件而自动接受新债务。

## 3. API examples 与文档

对每个公开 namespace 执行 examples：

```bash
calcit calcit.cirru analyze check-examples --ns package.api
calcit calcit.cirru analyze check-examples --ns package.extra
```

定位单个定义时加 `--def <definition>`。`No functions with examples` 且退出 0 只表示没有 example 覆盖，不是验收通过；公开 API 应补 runnable example 或由明确的项目测试覆盖。

验证 README 和 `docs/` 中的 Cirru 代码块：

```bash
calcit calcit.cirru docs format-md README.md --check
calcit calcit.cirru docs format-md docs/api.md --check
calcit calcit.cirru docs check-md README.md --failures-only
calcit calcit.cirru docs check-md docs/api.md --failures-only
```

`format-md` 只规范 fenced Cirru 的文本格式；`--check` 不会写入文件，适合 CI。需要改写时省略 `--check`。如果文档示例使用额外 module，可重复传 `--dep <module-dir>`。第一个表达式可以是带 `:require` 的 `ns`，由 `check-md` 注入 eval 上下文。

## 4. 入口、构建和行为

先做不执行业务入口的检查，再按 entry mode 验收：

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru
calcit calcit.cirru --entry test
```

默认运行是 once；只有热更新场景才使用 `-w` / `--watch`。entry 的 `:mode` 决定 native 运行或 JS 生成，因此项目脚本和 CI 应优先依赖统一 entry 配置。显式 `calcit calcit.cirru js` 只用于兼容或针对性 codegen 验证。

`--check-only` 会预处理所选 entry 的 `:init-fn` 与 `:reload-fn`；任何一个指向不存在或无法预处理的定义都应让验收失败。这能发现“正常启动暂时没走到 reload，所以旧配置被漏过”的问题。

类库若同时承诺 native 与 JS，应为两条链路配置独立 entry 或测试脚本。生成 JS 后还需执行真实 Node/Vite 测试，不能把 codegen 成功当成运行正确。

## 5. 可达性和公开 API

可以用 call graph 辅助发现无意遗留的定义：

```bash
calcit calcit.cirru analyze call-graph --show-unused --ns-prefix package
```

这是 entry-relative 报告。公开 API、外部回调、替代入口和由消费者调用的定义都可能显示为 unreachable；不得仅凭该报告自动删除。发布前应把每个命中分类为：真正 dead code、公开 API、替代入口、动态/FFI 调用，或缺失测试覆盖。

类库应另外显式检查每个承诺的运行目标。每条命令选择 shared namespace
和对应 target namespace，且 entry 必须配置同名 `:target`：

```bash
calcit --entry node calcit.cirru analyze check-public \
  --ns package.shared --ns package.node --format json
calcit --entry browser calcit.cirru analyze check-public \
  --ns package.shared --ns package.browser --format json
```

该检查枚举 namespace 中全部 definitions，因此未被测试入口引用的公开函数、
data declaration 和 external trait 也会预处理。空 scope、缺少 target、错误
runtime target 或任何诊断都会返回非零；不要用生成的引用函数或手写 export
清单替代它。普通 `--check-only` 仍只验证 entry 可达路径。

## 6. 真实消费者回归

CLI 查询、编辑、类型分析或公共 API 改动后，使用全局安装的新 `calcit` 在 Respo 等真实项目验证：

1. 让消费者指向待验收 module 版本或本地副本。
2. 运行消费者自己的 `--check-only`、entry、测试和目标 codegen。
3. 只需要大项目统计时使用 `--summary-only`。
4. 有写入风险的 `calcit edit` / `calcit tree` 先作用于 Snapshot 临时副本。
5. 记录消费者 commit、entry 和命令，避免只留下“手工试过”的结论。

## 7. 建议 CI 顺序

```bash
caps --ci
calcit calcit.cirru edit format
git diff --exit-code -- calcit.cirru
calcit calcit.cirru --check-only
calcit --entry node calcit.cirru analyze check-public \
  --ns package.shared --ns package.node --summary-only --format json
calcit calcit.cirru analyze check-types --summary-only --format json
calcit calcit.cirru analyze weak-types \
  --only schema-dynamic,unresolved-type-slot,code-dynamic \
  --intent unresolved \
  --summary-only \
  --format json
calcit calcit.cirru analyze check-examples --ns package.api
calcit calcit.cirru docs check-md README.md --failures-only
calcit calcit.cirru --entry test
```

在这条基础链路后追加仓库自己的 JS build、Node/Vite test、FFI build 和真实消费者 smoke test。已有 baseline 的存量类库可暂时追加 `analyze quality --baseline ...`，但它只是 0.14.x 迁移 ratchet；新类库不要采用。`unsafeCoerce` 的数量从来不能证明 runtime contract 已执行。

## 8. 发布前记录

PR 或 release note 至少记录：

- Calcit / `@calcit/procs` / module 版本。
- 验收过的 entries 和后端。
- 类型摘要与 unresolved baseline 变化。
- examples 和 Markdown 检查范围。
- 真实消费者项目与 commit。
- 已接受但尚未消除的动态边界及原因。

这样后续升级可以复用相同证据，而不是重新猜测“上次是否真的验证过”。
