---
title: "Calcit 项目升级手册（Respo / Lilac）"
summary: "当前升级闭环：核对版本、受保护的源码与附带区域迁移、严格检查及真实后端验证"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "upgrade"
  - "dependency migration"
  - "respo upgrade"
  - "lilac upgrade"
id: core/run/upgrade
related:
  - core/run/upgrade-history
  - core/run/library-quality
  - core/run/entries
  - core/features/static-analysis
---

# Calcit 项目升级手册（Respo / Lilac）

本手册面向需要从旧版 Calcit 逐步迁移到当前版本的项目。它不能保证旧代码无需修改就一次通过；
它保证的是：先保留可回滚基线，再用 Calcit CLI 把依赖、Snapshot、配置、类型和行为问题分层暴露，
每一层通过后再收紧下一层，避免把所有失败混在一次升级里。类库/module 发布前的完整证据矩阵见
[Calcit 类库项目验收与质量门禁](library-quality.md)。

## 当前升级闭环

本节是项目升级的唯一顺序。Agent 可直接运行 `calcit docs read upgrade.md '当前升级闭环'` 读取本节；
CLI 内嵌的 `calcit docs agents 项目升级入口` 只说明如何找到本节和三类动作的边界。按实际失败位置再查后文章节，
不需要先加载全部历史改名或审计报告，也不按版本猜 preset。

### 0. 确认本流程与目标工具链同版本

`docs read` 读取 `~/.config/calcit/docs` 指向的独立 checkout（目录约定见 [文档索引](../docs-indexing.md)），
安装新 CLI 不会同步更新它：

```bash
calcit --version
git -C ~/.config/calcit/calcit describe --tags
calcit docs agents --contract
```

guidebook 的 tag 应与目标 `calcit --version` 相同；不同时先 checkout 对应 release tag，或阅读该 tag 下的
`docs/run/upgrade.md`。`docs agents --contract` 的 `Agent source` 行给出 CLI 内嵌指南的版本。API 名称和签名以已安装
CLI 的 `query def` / `query type --format edn` 为准，旧 guidebook 不能作为新版本 API 的证据。

### 步骤、类别与证据

每个动作属于三类之一：**自动**（已证明等价，可按预览 revision 应用）、**需审阅**（项目级事务，审阅后才应用）、
**提示**（文档说明的行为变化或后端差异，没有唯一改法）。没有类别的步骤是检查或显式决定。

| # | 阶段 | 类别 | 使用的工具链 | 验收证据 |
| --- | --- | --- | --- | --- |
| 1 | 基线 | — | 项目当前固定的 CLI | 原 CI/脚本命令的实际结果与 `test --list`；已知失败单独记录 |
| 2 | 旧版迁移桥梁 | 自动、需审阅 | 项目当前固定的已发布 CLI | 桥梁规则的迁移 diff、重复预览为空、原测试 |
| 3 | 版本更新 | 显式决定 | `caps`、`yarn` | `deps.cirru`、`package.json`、lockfile 的 diff 单独提交 |
| 4 | 安装与工具链核对 | — | 目标 CLI、`caps` | `caps verify --toolchain`、mutation contract、`query config` |
| 5 | 表示规范化 | 自动 | 目标 CLI | `edit format` 的 diff 已审阅 |
| 6 | 源码迁移 | 自动 | 目标 CLI | strict workflow / preset 的 revision、`source-coverage`、重复预览无可应用建议 |
| 7 | 项目级事务 | 需审阅 | 目标 CLI | 审阅过的候选或模板、scope、暂存检查结果 |
| 8 | 严格检查 | — | 目标 CLI | 每个 entry 的 `--check-only`、`fix --workflow strict --verify` |
| 9 | 原附带断言 | — | 目标 CLI | 第 1 步的测试名仍在且 `--require-match` 通过；examples 与文档片段通过 |
| 10 | 目标后端 | 提示 | 目标 CLI、项目构建器 | 按 entry `:mode` 实际运行 native、JS 或已支持的 WASM/WASI |
| 11 | 提交 | 提示 | — | 宏生成引用、依赖模块、CI/文档中的旧调用已核对；最新 HEAD 的 CI/review |

### 1–2. 基线与旧版迁移桥梁（原工具链）

先用项目当前固定的 CLI 只读核对，并保存原测试清单：

```bash
git status --short
calcit --version
caps --version
calcit calcit.cirru query config --format edn
calcit calcit.cirru test --list
calcit calcit.cirru fix --workflow strict --format edn
```

再按 `.github/workflows/` 与 `package.json` 运行项目原有的检查和构建命令，记录结果。

**需要旧版迁移桥梁时，先迁移源码，再升级工具链。** 例如 0.28.x → 0.29.0，List `.join`、Map `.values`、
List/String `.contains?`，Number `.round?`、FsPath `.write-text`、FfiResponse `.resolve` / `.reject`，List `.reduce` / `.bind` / `.join-str` / `.nth`、Map/Set `.mappend`、Set `.add`、Map/Set `.contains?`、Map `.includes?`，以及函数 `some?`、`join-str`、String `.count` 的迁移规则在目标 CLI 中已退役或缩小范围，应先阅读本页“兼容入口的退场节奏”，用匹配项目的
0.28.x CLI 预览、携带 revision 应用并运行原测试。已发布的 0.28.x CLI 没有 `--include-attached` 与 `--pattern`，
桥梁只迁移 definition `:code`，预览的 `:manual-review-regions` 会列出 `|tests` 与 `|examples`；这两处的旧写法在第 9 步由目标
CLI 的 `E_RETIRED_METHOD` 等诊断定位后逐处改写。若已安装目标 CLI，可通过原已发布 CLI 的绝对路径执行旧阶段，
不修改版本 pin 来绕过校验。各桥梁的适用范围见下文 [历史迁移桥梁](#历史迁移桥梁)。

### 3–4. 版本更新、安装与工具链核对

依赖更新与安装由调用方明确执行：`caps upgrade --all` 会修改依赖选择与 `:calcit-version`，不是只读预览；只在已确认所有依赖均按该策略升级时执行。保留特定版本的项目按已审阅版本更新声明，然后安装。
详细说明见下文 Step A–E。

```bash
caps upgrade --all
caps
yarn install
caps verify --toolchain
yarn install --immutable
calcit docs agents --contract
calcit calcit.cirru query config --format edn
```

lockfile 与依赖未变时只需要 immutable 安装；没有 `package.json` 的 native 项目会由 `caps verify --toolchain`
报告跳过 `@calcit/procs` 核对。

### 5–7. 规范化与源码迁移

```bash
calcit calcit.cirru edit format
git diff
calcit calcit.cirru fix --workflow strict --format edn
```

Snapshot 已规范化时 format 不产生改动。按预览中的实际 revision 重跑同一 strict workflow；不要向 strict 命令传入 selector 参数：

```bash
calcit calcit.cirru fix --workflow strict --apply --expect-revision '<已审阅 manifest 的 revision>' --format edn
```

迁移动作按以下三类处理：

- **可证明自动迁移**：strict workflow 只应用报告中 `:safe-fixes` 的候选，并沿用预览 revision。需要显式选择规则或 preset
  （包括附带区域）时，使用独立的规则/preset 路径，例如 `calcit calcit.cirru fix --preset core-api-0.28-v1 --include-attached --format edn`，
  在 apply 时保留相同 selectors 和 revision；以 `:source-coverage` 的 `:scanned-regions` 判断范围，重复预览应没有可应用建议。
- **项目级需审阅事务**：workflow 的 `:review-required`（`:source-fixes`、`:type-findings`、`:ffi-boundaries`、
  `:schema-candidates`、`:structural-candidates`）与顶层 diagnostics 给出位置；旧 helper 或调用形状使用
  [项目级结构改写](fix.md#项目级结构改写)。`--pattern` / `--replace` 的所有候选都需审阅，暂存严格检查不能证明业务语义等价。
  不传 `--ns` / `--def` 时暂存检查覆盖整个项目，其他定义尚未迁移的 warning 会让整批事务被拒绝；先处理这些 producer，
  或把模板限定在已审阅的定义。schema、decoder、FFI 信任和业务默认值由实际合同决定。
- **仅文档提示**：行为变化、旧桥梁的适用工具链和未支持的后端由本页对应章节说明；没有唯一等价改法时保留明确待办，不猜 replacement。

### 8–10. 严格检查、原附带断言与目标后端

```bash
calcit calcit.cirru --check-only --keep-going --format edn
calcit calcit.cirru --entry '<entry-name>' --check-only
calcit calcit.cirru fix --workflow strict --verify --format edn
calcit calcit.cirru test --list
calcit calcit.cirru test --require-match
calcit calcit.cirru analyze check-examples --ns '<namespace>' --def '<definition>'
calcit calcit.cirru docs check-md README.md --dep ./
calcit calcit.cirru --entry '<entry-name>'
```

named entry 不继承 default 配置，逐个检查与运行。把 `test --list` 与第 1 步的清单比较，原测试名必须仍在；新增测试不能代替原断言。
最后一条按 entry 的 `:mode` 执行 native 或生成目标产物：JS 项目继续运行生成代码的 Node/Vite 测试，WASM 用 `calcit wasm` /
`calcit wasi` 的已支持路径，依赖 native 模块的项目先构建对应 dylib。类库还要运行公开 namespace 检查和真实消费者回归，
见 [类库项目验收](library-quality.md)。

### 结构化输出

显式请求结构化数据时使用 `--format edn`：Cirru EDN 保留 tag、symbol 等原生值语义，strict workflow manifest、`fix`、
`query`、`--check-only --keep-going` 与 `analyze weak-types` / `analyze verify` 都提供它。`--format json` 是同一 envelope
的兼容表示，只用于 JSON-only 工具。`calcit test`、`analyze quality`、`analyze check-public`、`analyze check-types` 与
`analyze deprecated` 目前只提供 human 和 JSON；在这些命令上使用 JSON，不为格式差异另建入口。

### 历史迁移桥梁

固定桥梁是临时改写，不是按版本维护的规则表。下表只列出已有桥梁；其余旧写法由当前诊断与当前 AST 规范化处理。

| 桥梁 | 适用的旧写法 | 使用的 CLI | 状态与退场条件 |
| --- | --- | --- | --- |
| `compact.cirru` → `calcit.cirru` | 早期双文件 Snapshot、direct quote/configs | 0.13.48 及更早确认基线；当前 `edit format` 的一次性读取 | 当前仍可用；见 [快照文件迁移说明](#快照文件迁移说明) |
| `tag-match-to-match-v1`、`required-struct-field-v1` | 0.14.15 之前的 tag match 与可缺失 Struct 字段 | 已发布 0.14.15 | 已退役；见 [历史版本迁移记录](upgrade-history.md) |
| `core-list-intersperse-v1`、`core-map-distinct-values-v1`、`core-predicate-method-v1` 的 List/String 部分 | List `.join`、Map `.values`、List/String `.contains?` | 已发布 0.28.x | 0.29.0 已退役；见 [兼容入口的退场节奏](#兼容入口的退场节奏) |
| `core-integer-predicate-v1` 的 Number 方法部分、`core-effect-method-v1` 的 FsPath/FfiResponse 部分 | Number `.round?`、FsPath `.write-text`、FfiResponse `.resolve` / `.reject` | 已发布 0.28.x | 0.29.0 已退役；见 [兼容入口的退场节奏](#兼容入口的退场节奏) |
| `core-set-include-v1`、`core-list-fold-v1`、`core-list-flat-map-v1`、`core-list-join-string-v1`、`core-list-get-v1`、`core-collection-combine-v1` | List `.reduce` / `.bind` / `.join-str` / `.nth`、Map/Set `.mappend`、Set `.add` | 已发布 0.28.x | 0.29.0 已退役；见 [兼容入口的退场节奏](#兼容入口的退场节奏) |
| `core-predicate-method-v1` 的 Map/Set 部分 | Map `.contains?` / `.includes?`、Set `.contains?` | 已发布 0.28.x | 0.29.0 已退役，整条规则不再提供；见 [兼容入口的退场节奏](#兼容入口的退场节奏) |
| `core-non-nil-predicate-v1`、`core-collection-len-v1` 的 String 部分 | 函数 `some?`、String `.count` | 已发布 0.28.x | 0.29.0 已退役；见 [兼容入口的退场节奏](#兼容入口的退场节奏) |
| `core-api-0.28-v1` 与各兼容名的 fix rule | 0.28 起的核心 API 旧名 | 目标 CLI | 当前可用；按 [兼容入口的退场节奏](#兼容入口的退场节奏) 退场 |

退场条件统一为：已知活跃下游默认分支的源码、附带测试/示例、宏生成代码和 CI/文档引用清零，且依赖模块与未合并迁移已核对后，
在下一个非 patch 版本删除，并在本页列出删除项与迁移命令；扫描失败、未覆盖或仅语法零命中不算清零。

### 限制

- strict workflow 的当前规则组合不支持整体 `--include-attached`；附带区域须使用已支持的显式规则/preset 并另行验证。
- 已发布的 0.28.x CLI 不支持 `--include-attached` 与 `--pattern`。
- 预览因源码或依赖类型错误失败时先修复真实 producer；没有有效 manifest 和 revision 时不继续 apply。
- `requires-review` 候选与严格类型错误分别处理：workflow 未通过时读取具体 gate；已证明的变长调用（包括注册宿主 proc 的 List spread）不再列为待审，固定参数不足、开放实参或展开后不唯一的 spread 仍需人工审阅；不为清空报告改成固定参数、扩大 Dynamic 或删除门禁。
- manifest 中未执行的 external gates 和零测试匹配都不是通过证据。
- workflow 顶层 diagnostics 中位于 `calcit.core/*` 的证明错误来自 bundled core，不是项目可写源码；附最小复现报告到 Calcit 核心仓库，不在项目中绕过。
- 版本不匹配时应安装项目固定 CLI 或显式升级合同，不绕过工具链 pin。
- 已退役规则不能用目标 CLI 重新获得；先用对应旧版桥梁，或按具体诊断人工迁移，不反复更换 preset 猜测。
- `--check-only`、生成成功或零语法命中不能代替目标后端运行及真实消费者回归。

## 显式 trait 调用的重复实现

`&trait-call Trait :method receiver ...` 按 trait 来源选择实现。同一个接收者上，该来源必须只有一个候选；多个候选会报 `E_DUPLICATE_TRAIT_IMPL`，不会执行其中任何一个方法。native 与 JS runtime 均执行这条规则，包括显式 `--compat-types` 生成或运行的调用。

严格源码原本已拒绝此类重复实现；此次收紧补齐运行时边界。此前兼容模式对 Struct/Enum 选择最后一个候选、对内建接收者选择第一个候选。升级时删除同一 trait 来源的重复 attachment，或使用不同 trait 表达不同能力，再显式选择需要的 trait。不能仅交换 impl 顺序来修复，也没有可自动决定保留哪种业务行为的 fix。

不同来源 trait 的同名方法仍可通过 `&trait-call` 消歧。此变更不调整兼容模式中普通 `.method` 的历史查找顺序，也不增加新的运行时配置开关。

## `recur` 的尾部位置

`recur` 必须在函数体或 `loop` 体的尾部位置直接调用。尾部位置沿 `if` 分支、`let` / `do` 的最后一个表达式、`match` 分支体和 `try` 的主体传递；宏展开后产生的 `recur` 按展开结果同样检查，`calcit.core` 也不例外。

```cirru
loop ((a 1))
  try
    if (> a 3) a $ recur $ inc a
    fn (e) 0
```

以下写法在检查阶段报错：把 `recur` 的结果放进集合或作为参数、放在非最后一个表达式中、用作 `if` 条件或 `let` 绑定值，以及把 `recur` 本身作为值绑定或传递（如 `let ((r recur)) ...`、`apply recur xs`）。此前 `try` / `match` 内部的这类写法会通过检查，运行时把 `Recur` 当作普通数据或直接丢弃。升级时把 `recur` 移到尾部，或改为普通递归调用；原写法的行为本身有误，没有自动 fix。

native runtime 遇到逃逸的 `Recur` 值（出现在参数、绑定、条件、被丢弃的表达式或定义值中）会报错；JS 与 WASM codegen 对非尾部 `recur` 报错，不再生成会泄漏数据或静默重启循环的代码。

## `str-spaced` 的首参数与格式化边界

`str-spaced` 的签名明确为 `(x0 & xs)`：至少传一个值，跳过 nil，以空格连接其余值的文本。单个 nil 或全 nil 仍返回空字符串，空字符串本身仍占据一个拼接位置。内部拼接函数只接收 String；异质值的转换保留在公开格式化边界，不再让 nullable 泛型进入拼接过程。

此前公开签名是纯 rest，但零参数调用实际会在内部函数缺少 `x0` 时报错。现在提前拒绝同一无效调用，不为它增加空字符串默认值。普通非空调用无需改写。

```cirru
assert= "|a 12 false" $ str-spaced |a nil 12 false
assert= | $ str-spaced nil nil
assert= "| b" $ str-spaced | |b
```

只有未知长度列表的展开调用需要审阅：`str-spaced & xs` 不能证明首项存在。应先由调用者确认非空，再显式传首项与剩余项；空列表如何处理属于业务决策，不自动插入 fallback、unsafe 或长度断言。此签名收敛安排在非 patch 升级中，不为这一情形新增 fix 规则。

## 嵌套 Struct 构造的字段合同

直接构造器与 `%{}` 在参数完成类型推导和 lowering 后检查相同的字段合同。递归声明中的名义类型同样生效：字段声明为 RenderNode Enum 时，不能直接放入 Element Struct；`List<ChildPair>` 不能用原来的二元 List 替代。应显式构造声明要求的 Enum 变体或 ChildPair 值，不能只增加 `assert-type` 或 `unsafe-coerce` 来绕过错误。

空 Enum 变体会保留未承载值的泛型信息。先将 `Option :none` 绑定到局部变量、再次取别名或作为 `if` 分支，再用于具体 Option 字段，同样合法；普通 Calcit import 的薄构造 wrapper 也适用。这项推导依据实际变体，不依赖 `Option` 名字。返回 schema 为 `Option<Dynamic>` 且实际变体可能承载开放值的函数，仍不能证明具体 payload 类型。

可变 Ref 的 payload 合同需要覆盖后续写入，而不只是初始空值。用 `ref` 创建将来会保存具体值的 Ref 时，在初始化表达式中明确该合同，例如 `ref $ assert-type (Option :none) $ :: 'Option 'Number`。这里验证的是合法空变体到声明类型的关系，不是给已有 Ref 强转类型；后续 `reset!` 仍会拒绝不符合 Number 合同的 payload。未补充上下文的空值不会自动变成可写入任意类型的 Dynamic Ref。缺少该上下文时，`reset!` / `swap!` 处的 `W_RESET_ARG_TYPE_MISMATCH` 会附带上述初始化写法；Ref 的类型不按后续写入推导，修复位置是初始化表达式而不是写入点。

这项修复不改变合法构造的字段求值顺序，也不把静态检查变成外部数据校验。开放输入仍需在原有 decode/验证边界处理；如何把业务值转为闭合节点属于人工迁移，不提供自动默认值或改写。

## Ref 写入的返回值

`reset!` 返回写入的新值，`swap!` 返回其更新函数产生并写入的值；`!` 表示副作用，不表示返回 Unit。
返回类型从赋值表达式推导，保留泛型与名义类型，不借 Ref 的 payload 声明把开放输入变成具体值。

```cirru
let
    counter $ ref 1
  assert= 2 $ reset! counter 2
  assert= 3 $ swap! counter inc
  assert= 3 $ deref counter
```

声明 `Fn() -> Unit` 的业务函数若末尾是 `reset!` 或 `swap!`，应在所有操作完成后显式返回 `&unit`，
而不是更改写入语义或增加 unsafe。未知输入仍需在原边界收窄；错误 payload 写入继续被拒绝。
此推导修复不改变 native/JS/WASM 既有运行值，也不扩展 WASM 的 Ref 支持范围。

## 取余的跨后端语义

`&number:rem` 和 Number `.rem` 在 native、JS、core WASM 与 WASI Component 上使用同一契约：两个操作数都必须是安全整数（绝对值不超过 `9007199254740991`），除数不能为 0（包括 `-0`）；结果为截断取余，符号跟随被除数，且不返回 `-0`。

```cirru
assert= -1 $ &number:rem -7 3
assert= 1 $ .rem 7 -3
assert= 4 $ &number:rem 4294967296 7
assert= "|&number:rem requires safe integers, but received: 5.5 2" $ try (&number:rem 5.5 2)
  fn (error) error
```

native 与 JS 对越界输入抛出可由 `try` 捕获的相同错误：零除数为 `&number:rem divisor must not be zero`，小数、NaN、Infinity 或超出安全整数范围为 `&number:rem requires safe integers, but received: <a> <b>`，操作数按 Calcit 数值文本输出（`inf`、`-inf`、`NaN`，大数与极小数展开为十进制）。WASM 对同样的输入 trap。完整规则见 [Number](../data/number.md#取余-rem)。

升级时的行为变化：

- JS/WASM 此前对小数做浮点取余（`rem 5.5 2` 为 `1.5`），零除数返回 NaN；现在两者都报错。依赖小数取余的代码需要改为显式的浮点计算，例如基于 `floor` 的 `&- a $ &* b $ floor (&/ a b)`，舍入方式由业务决定。
- native 此前把超出 i32 的整数饱和截断后取余（`rem 4294967296 7` 得到 `1`），`-2147483648` 除以 `-1` 报溢出错误；现在按安全整数精确计算，分别得到 `4` 和 `0`。
- native 此前按 EPSILON 容差把非常接近整数的小数当作整数；现在要求恰好没有小数部分。
- 0.29 的早期预览版中，JS 错误消息用宿主文本输出操作数（`Infinity`、`1e+21`），与 native 的 `inf`、`1000000000000000000000` 不一致；现在两者相同。按消息文本匹配非有限值或大数的 `try` 处理需要改用 Calcit 文本。

这些都是语义修复，不提供自动源码改写；需要错误恢复的业务自行处理 `try` 的错误，不自动补默认值。

## 整数谓词的跨目标语义修复

`round?`（以及 0.28 的 Number `.round?`，该方法已在 0.29.0 删除，见下文）现在统一判断“有限且恰好没有小数部分”。native、JS、core WASM 和 WASI Component 使用相同契约：NaN、正负 Infinity、任何非零小数均为 false；0、-0 与有限的整数值为 true。

此前 native 使用 EPSILON 容差，把 `0.0000000000000001` 等近零非整数误判为 true；JS/WASM 则把 Infinity 误判为 true。这是行为修复，不是批量 rename。依赖旧容差的业务需要显式选择适合其精度的近似比较；不要通过自动 fix 猜测容差或加入舍入。新代码首选已提供的 `integer?/.integer?`；函数 `round?` 暂留同义兼容，可用 `calcit fix --rule core-integer-predicate-v1` 显式迁移已证明的调用，详见 [fix 规则](fix.md)。

该 Bool 谓词不等于“安全整数”或 `Int32/UInt32` 等数值 refinement 证明。例如 `9007199254740992` 仍是有限且无小数部分的 Number，所以结果为 true；需要特定边界时继续使用对应的 checked 转换。这次不改变索引转换、JSON 格式化的内部容差，也不改变 `round` 的舍入行为。

```cirru
assert= false $ round? 0.0000000000000001

assert= false $ .integer? $ / 1 0

assert= false $ .integer? $ / 0 0

assert= true $ .integer? -0

assert= true $ round? 9007199254740992
```

## Struct 数据字段写入检查

命名 Struct 的 `assoc` / `with` 写入会校验可解析的数据字段合同：List、Map、Set 的成员、Optional、Enum payload，以及嵌套 Struct 的字段与声明来源。字段声明带具体泛型实参时按该实参检查，例如 `Box<Number>` 的 List 字段不能写入 String 元素。声明中的显式 `Dynamic` 叶子仍可保存开放数据。

JS 的 `&struct:assoc` 写入不存在的字段会抛错，与 native 一致；不再返回未修改的原值。非法值也会抛错，原 Struct 保持不变。调用方可以按现有异常边界处理失败，不应依赖 silent no-op，也不应将名义来源不同的同名 Struct 当成可互换值。

构造与更新使用同一字段检查。静态类型已知时继续使用 `Person :name |Ada` 等直接构造器；动态 StructDef 使用 `%{} prototype (:name value)`，其可解析数据字段也会检查嵌套内容。低层 `&struct:from-map` 只接受 Map，必须提供每个字段且恰好一次：Tag/String 归一化后重复、缺少、多余或类型不匹配都会抛错。native 不再给遗漏字段填 nil，JS 不再忽略多余字段或接受 Struct 作为 Map；先显式生成完整 Map，不依赖这些跨后端差异。零字段 Struct 的定义与完整构造在 native/JS 一致接受。

动态 `&struct:assoc` / `&struct:with` 只有在运行时能够验证字段合同时才完成更新，因此可作为严格检查认可的受检边界。未实例化的泛型、带参数/返回合同的函数、以及包含具体 payload 类型的可变 Ref，无法仅凭当前值建立证明时会抛错。显式开放的 `Dynamic`、裸 `Fn`、`JsObject` 和 `Ref<Dynamic>` 保留各自声明的开放语义，不等于为其他字段放宽类型。

可空字段的缺席分支独立成立：`Optional<T>` 可以写入 `nil`，`JsNullish<T>` 还允许 JS 宿主的 `undefined`。例如 `Optional<Ref<Number>>` 可以清空为 `nil`；非空 Ref 或带签名函数仍须证明内部合同，不能借可空声明放行。

类型已知的更新仍使用普通 `assoc` / `.assoc`，让编译器保留泛型、函数字段及 Ref 的静态证据并完成 indexed lowering；已知 String 字段名也按同一路径处理。不要为通过检查改写成 `&struct:assoc-at` / `&struct:with-at`，手写 indexed 调用仍必须证明原字段合同。

### 限制

- 构造入口对未实例化泛型/函数仍保留原静态合同与运行时 fallback，不能当作任意 Dynamic 的完整转换证明。
- JS 深层合同由编译器为命名定义携带；动态生成的 prototype 不因此获得完整字段证明。
- 此项修复不扩展 WASM/WASI 对动态 Struct 更新的支持范围。

## 短路条件保留类型证据

`if` 的条件经过宏展开后，编译器从实际的分支与局部绑定推导类型证据。例如 `and` 同时检查值是 Number、整数且位于列表范围内，成功分支可直接使用这个索引：

```cirru
defn replace-second (k)
  if
    and (number? k) (= k $ floor k) (&>= k 0) (&< k 2)
    &list:assoc ([] 1 2) k 3
    [] 1 2
```

参数可以保留 `Dynamic`，返回合同为 `List<Number>`；成功分支中的 `k` 已有 Number 证明。无需为了通过严格检查把条件拆成嵌套 `if`，也不需要插入强转。此检查不改写程序，保留短路顺序、求值次数和返回值。

多个变量可同时获得证据；`or` 的成功分支只保留每条可达路径都有的证据，`or (number? k) true` 不能证明 `k` 是 Number。假分支也按实际路径分析。

### 限制

- 任意返回 Bool 的用户函数不是类型谓词；同名用户定义不能冒充 core 谓词。
- 未知函数调用、局部变量写入与词法遮蔽会阻断受影响的证据；调用前的证明不能无条件沿用到调用后。
- 跨条件保存任意布尔变量尚不形成完整的数据流分析；必要时在具体使用前直接检查原值。
- 这项共同预处理修复不扩大 WASM 对开放值或集合的支持范围。

## count 的 Countable 合同

本节的合同加强面向 0.29 非 patch 升级；已发布旧工具链仍以其实际 `query def/type` 为准，不把开发分支的行为当作旧版本保证。

`count` 要求参数具备 `Countable` 能力，返回 `Number`。List、Map、Set 和 String 保持原有长度语义；Struct 计字段数，Enum 的计数包含 tag。String 按 Unicode 标量计数，不是 UTF-8 字节数或 JavaScript 的 UTF-16 长度。已知集合类型也可以使用其 `.len` 方法；String 的 `.count` 方法已在 0.29.0 删除，函数 `count` 与 `Countable` 约束仍接受 String。

泛型 helper 调用 `count` 时，需要在 Fn schema 的 `:where` 中声明 `T Countable`；名义 Struct/Enum 参数由编译器解析其声明并证明已有能力，无需改用低层计数原语。开放 `Dynamic` 在使用内容前须显式收窄，例如在 `list?`、`map?`、`set?` 或 `string?` 成功分支中计数。`nil` 不作为空集合，Number 和函数也不是可计数值。

这项收紧不是等价改名，不提供自动插入类型转换、默认空集合或 `unsafe-coerce` 的 fix。升级时保留原有断言与失败行为，根据业务边界补充合同或类型谓词，再运行严格检查、定义附带测试和目标后端构建。

trait 的 where 约束现在使用能力证明，而不是把未绑定泛型静默当作通过。若 helper 约束的是 Enum 的 payload 类型，空变体没有 payload 可供推断时，应补充确切的类型。例如已知接下来使用 `Maybe1<Number>`，可对直接空构造器写 `assert-type (Maybe1 :none) (:: 'Maybe1 'Number)`；这是编译期验证空构造器的类型，不是强转开放 Dynamic。单纯 `count (Option :none)` 约束的是整个 Enum 的计数能力，仍不需要补 payload 类型。

`loop` 初始值和 `apply` 的无 spread 字面量实参保留各位置的类型，立即调用的固定参数函数可从输入推断参数。显式 `hint-fn` 优先，不能因为调用点传入 List 就把已声明的 Dynamic 参数悄悄改成 List 或 Countable；这种开放合同仍须在函数内部收窄。合法 typed loop 不需要为了通过检查改用低层计数或重复插入断言。

## NaN 的相等、排序与哈希

`=`、`&compare`、`sort`、Set 成员与 Map 键在 native、JS 与 WASM 上对数字使用同一组规则：`NaN` 等于 `NaN`，`0` 等于 `-0`；`&compare` 与默认比较的 `sort` 把 `NaN` 排在所有数字（包括 `inf`）之后；相等的数字哈希相同，`NaN` 可以作为 Set 元素或 Map 键找回。

```cirru
let
    not-a-number $ sqrt -1
  assert= true $ = not-a-number not-a-number
  assert= 1 $ &compare not-a-number $ &/ 1 0
  assert= true $ contains? (#{} not-a-number) not-a-number
```

升级时的行为变化：

- 此前 `(= x x)` 对 `NaN` 为 `false`，有代码借此判断 `NaN`；现在结果为 `true`。需要判断 `NaN` 时改用 `&= 1 $ &compare x $ &/ 1 0`。
- 此前含 `NaN` 的列表排序结果无序，`NaN` 放进 Set/Map 后查不到；现在 `NaN` 固定排在最后，查找能命中。
- `<`、`>`、`<=`、`>=` 不变，仍按 IEEE 754 在 `NaN` 参与时返回 `false`。

这是语义修复，不提供自动源码改写。完整规则见 [Number](../data/number.md#相等排序与哈希)。

## 宿主句柄不参与值比较与哈希

已声明为 external-object trait 的值在 `=`、`not=`、`&=` 中比较，或作为 Set 成员、Map 键使用时，检查报告
`W_HOST_VALUE_EQUALITY`，严格模式下构建失败。运行时宿主对象只按引用身份比较，旧写法得到的也只是身份结果。

迁移时按原意选择：判断是否同一个宿主对象改用 `identical?`；按内容比较或建立索引时，先在适配器内取出 id、
文本等 Calcit 数据再比较。这一检查不提供自动改写，裸 `JsObject` 与 `Dynamic` 值不受影响。

## String 到 Tag/Symbol 的类型化转换

新代码用 `to-tag: String -> Tag`、`to-symbol: String -> Symbol`。旧 `turn-tag` / `turn-symbol` 的 native/JS 运行时还接受 Tag/Symbol 输入，不能按词形全局替换。`calcit fix --rule core-identity-conversion-v1 --format edn` 只对内建调用且参数已证明为 String 的稳定源码提供自动迁移；Dynamic、非 String、quote、未知 macro 和一等函数引用需人工审阅。预览后携带 revision 应用，再重复预览并运行项目测试。WASM 缺少动态 Tag/Symbol intern；改名不会让原来不支持的调用变得可编译。

同一显式 fix 也能迁移部分旧 `turn-string` 调用：仅当参数已证明是内建 Nil、Bool、Number、String、Tag 或 Symbol，且源码上下文稳定时，改为首选 `to-string`。旧 `turn-string` 现通过 `ToString` trait 约束调用；真正的标量 primitive `&turn-string` 仅供内建实现使用。这使 List/Map/Unit/开放 Dynamic 在严格预处理时被拒绝，而不是等运行时报错；用户自定义 `ToString` 仍可经旧入口转换。自动 fix 继续只迁移六类标量，避免把未知实现差异误判为等价。

## WASM 运行时 Number 转文本的边界

已发布的正式 [0.28.0](https://github.com/calcit-lang/calcit/releases/tag/0.28.0) 包含运行时 Number 的精确文本格式化与已证明的 trait 调用，不需要等待 0.29.0。core WASM 与 WASI 0.3 对这些 `f64` 值使用与 native/JS 一致的最短十进制规则；`to-string` 的 Number 方法通过已证明的 `ToString` trait 实现进入同一运行时路径，不是新增的名称特判或 host import。

这项能力首次随 [0.28.0-alpha.1](https://github.com/calcit-lang/calcit/releases/tag/0.28.0-alpha.1) 发布。更早的 0.28 开发安全修复阶段，旧 `turn-string` 只保证绝对值不超过 2^53 的有限整数，小数、负零、超范围整数和非有限值会陷阱；不要把这个历史限制当作正式 0.28.0 的支持范围。

这也修正了 native 与 JS 在极少数 `f64` 最短表示末位上的分歧：统一采用 JS/Ryū 的选择。Number 二进制值和解析规则不变，但依赖数字文本逐字节相等的缓存键、快照或外部协议应在升级时重新比对。需要明确显示 Calcit 值或序列化结构数据时，分别使用 Debug/Show 或 Cirru EDN，不要把 `to-string` 当作通用序列化。

## 索引、键、值与成员谓词

`contains?` 的旧方法形式依接收者表示不同命题，不能全局替换：List/String 检查索引，Map 检查键，Set 检查成员；Map 的旧 `.includes?` 则检查值。新代码首选 List/String `.contains-index?`、Map `.contains-key?` / `.contains-value?`，Set 成员保留 `.includes?`。String `.includes?` 判断子串，List `.includes?` 判断元素，均不得与索引判断混用。

List/String/Map/Set 的旧 `.contains?` 与 Map 的旧 `.includes?` 已在 0.29.0 删除（见下文“兼容入口的退场节奏”），写下它们会得到 `E_RETIRED_METHOD`。下面描述 0.28.x 上的迁移：已证明旧新方法同属 core 实现时，用 0.28.x 的 CLI 显式预览 `calcit calcit.cirru fix --rule core-predicate-method-v1 --format edn`，审阅建议和 revision 后再应用。规则不迁移 Struct/Enum、`Contains` trait、自定义同名方法、开放接收者或 attached `:tests` / `:examples`；这些位置需要人工检查。Struct/Enum 的 `.contains?` 在 0.29.0 仍可用。完整边界见 [API 命名角色](../features/api-roles.md#谓词与成员查询) 与 [fix 规则](fix.md)。

## 兼容入口的退场节奏

维护者决定：非 patch 版本允许 breaking change，用于废弃旧的不合理用法；patch 版本不删除入口，也不新增会让严格检查失败的诊断。流程是先发布新入口与 guarded fix rule，再在下一个非 patch 版本删除旧入口，并在本文列出删除项与迁移命令。

剩余集中迁移与旧入口的计划退场窗口为 **0.30.0**，不撤回下文列出的 0.29.0 已删除项。到达计划版本仍须逐项核验实际消费者、已发布迁移窗口、类型与失败/求值语义，以及 core 方法实现解耦；源码、附带测试/示例、宏生成代码、锁定模块和未合并迁移都在核对范围内。只扫描到零调用、fix 预览为空或更新基线文件都不能代替这些证明。未满足条件的兼容入口继续保留，具体迁移仍按下面的旧新映射与当前 CLI 查询处理。

### 0.29.0 已删除：`.join`、Map `.values`、List/String `.contains?`

下面三组旧方法在 0.28.0 按 Rust/Clojure 习惯书写时能通过类型检查，但结果与直觉不符，已在 0.29.0 删除。写下它们会得到 `E_RETIRED_METHOD`，信息里给出首选替代；List 与 String 同时不再实现 `Contains` trait；Map 与 Set 也随后移出（见下文），需要该 trait 的泛型约束只剩 Struct 与 Enum，其余接收者请直接调用具名方法。

| 已删除 | 0.28 的行为 | 首选替代 | 迁移（使用 0.28.x 的 CLI） |
|---|---|---|---|
| List `.join` | 返回插入分隔符的 List，放入 `str` 也不报错 | `.intersperse`；字符串用 `.join-string` | `calcit calcit.cirru fix --rule core-list-intersperse-v1 --format edn` |
| Map `.values` | 值重复时只剩去重后的 Set 元素 | `.distinct-values` | `calcit calcit.cirru fix --rule core-map-distinct-values-v1 --format edn` |
| List/String `.contains?` | 判断下标，不判断成员 | `.contains-index?`；成员判断用 `.includes?` | `calcit calcit.cirru fix --rule core-predicate-method-v1 --format edn` |

三条迁移规则随旧方法一起退役：0.29.0 的 CLI 不再提供 `core-list-intersperse-v1`、`core-map-distinct-values-v1`，`core-predicate-method-v1` 的 Map/Set 部分也已退役（见下文），`core-api-0.28-v1` preset 因此先由 15 条规则减为 13 条。升级顺序是先在原工具链（0.28.x）上预览并应用这些规则，审阅 revision、再次预览并运行项目测试，然后才升级依赖和 CLI；规则只处理接收者类型已证明的方法调用，函数形式的 `join` / `vals`（仍可用，改名由 `core-function-alias-v1` 负责）、Dynamic 接收者与一等函数引用需要人工审阅。来不及迁移的项目会在严格检查时看到 `E_RETIRED_METHOD`，按提示逐处改名即可。

函数形式 `contains?` 不变：List 按下标、String 按下标、Map 按键、Set 按成员，调用方式与结果同 0.28。函数形式 `includes?` 也不变，Map 仍按值判断。

### 0.29.0 已删除：同实现的方法别名 `.round?`、`.write-text`、`.resolve`、`.reject`

下面四个方法别名与首选方法指向同一实现、同一类型契约，0.28.0 起已有对应的首选写法与 guarded fix rule，已知活跃下游默认分支没有真实调用，因此在 0.29.0 删除。写下它们会得到 `E_RETIRED_METHOD`，信息里给出首选替代。函数 `round?` 不受影响，仍可调用。

| 已删除 | 首选替代 | 迁移（使用 0.28.x 的 CLI） |
|---|---|---|
| Number `.round?` | `.integer?` | `calcit calcit.cirru fix --rule core-integer-predicate-v1 --format edn` |
| FsPath `.write-text` | `.write-text!` | `calcit calcit.cirru fix --rule core-effect-method-v1 --format edn` |
| FfiResponse `.resolve` / `.reject` | `.resolve!` / `.reject!` | 同上 |

0.29.0 的 CLI 中，`core-integer-predicate-v1` 只改写 reader 解析为内建 Proc 的 `round?` 函数调用；`core-effect-method-v1` 只保留 FfiTask `.cancel` / `.cancel-with`。两条规则仍在 `core-api-0.28-v1` preset 中，preset 规则数不变。升级顺序同上：先在 0.28.x 上预览、应用并运行项目测试，再升级 CLI。

### 0.29.0 已删除：List/Map/Set 方法别名与 `foldl'`

下面的旧入口在 0.27.0 / 0.28.0 正式版中已有首选写法，方法别名也已有 guarded fix rule；已知活跃下游默认分支没有调用，因此在 0.29.0 删除。除 Map `.add` 外，旧方法与首选方法指向同一 core 实现。写下这些方法会得到 `E_RETIRED_METHOD`，信息里给出首选替代；`foldl'` 已从 core 删除，按未定义名字报错。

| 已删除 | 首选替代 | 迁移（使用 0.28.x 的 CLI） |
|---|---|---|
| List `.reduce` | `.fold` | `calcit calcit.cirru fix --rule core-list-fold-v1 --format edn` |
| List `.bind` | `.flat-map` | `calcit calcit.cirru fix --rule core-list-flat-map-v1 --format edn` |
| List `.join-str` | `.join-string` | `calcit calcit.cirru fix --rule core-list-join-string-v1 --format edn` |
| List `.nth` | `.get`，同样返回 `Option<T>` | `calcit calcit.cirru fix --rule core-list-get-v1 --format edn` |
| Set `.add` | `.include` | `calcit calcit.cirru fix --rule core-set-include-v1 --format edn` |
| Map `.mappend` / Set `.mappend` | `.merge` / `.union` | `calcit calcit.cirru fix --rule core-collection-combine-v1 --format edn` |
| Map `.add [key value]` | `.assoc key value` | 人工改写：旧方法只检查 entry 长度，不证明键值类型，没有 fix rule |
| 函数 `foldl'` | `fold`，参数顺序相同 | 人工改写 |

上表的 6 条迁移规则随旧方法一起退役：0.29.0 的 CLI 不再提供 `core-set-include-v1`、`core-list-fold-v1`、`core-list-flat-map-v1`、`core-list-join-string-v1`、`core-list-get-v1`、`core-collection-combine-v1`，用 `--rule` 指定它们或更早退役的 `core-list-intersperse-v1`、`core-map-distinct-values-v1` 时，CLI 会提示改用 0.28.x 的 CLI。升级顺序同上：先在 0.28.x 上预览、应用并运行项目测试，再升级 CLI。

List/String/Fn 的 `.mappend`、String/Enum 的 `.nth` 与 Fn 的 `.bind` 不受影响。

### 0.29.0 已删除：Map/Set `.contains?` 与 Map `.includes?`

Map 的两个谓词方法名分别检查键和值，Set `.contains?` 与 `.includes?` 是同一实现；0.28.0 起已有具名的首选方法与 guarded fix rule，已知活跃下游默认分支没有调用，因此在 0.29.0 删除。写下它们会得到 `E_RETIRED_METHOD`，信息里给出首选替代。

| 已删除 | 0.28 的行为 | 首选替代 | 迁移（使用 0.28.x 的 CLI） |
|---|---|---|---|
| Map `.contains?` | 判断键 | `.contains-key?` | `calcit calcit.cirru fix --rule core-predicate-method-v1 --format edn` |
| Map `.includes?` | 判断值，不判断键 | `.contains-value?` | 同上 |
| Set `.contains?` | 判断成员，与 `.includes?` 同一实现 | `.includes?` | 同上 |

Map 与 Set 同时不再实现 `Contains` trait。schema `:where` 中 `'T 'Contains` 的泛型约束现在只接受 Struct 与 Enum；需要同时接受 Map 或 Set 的泛型代码请改为具体类型参数，或在调用处直接使用上表的具名方法。函数形式 `contains?` 与 `includes?` 不变，仍按 0.28 的语义接受 Map 与 Set，包括静态类型未知的接收者。

`core-predicate-method-v1` 随之整条退役：0.29.0 的 CLI 用 `--rule` 指定它时会提示改用 0.28.x 的 CLI。连同上一节的 6 条规则，`core-api-0.28-v1` preset 由 13 条规则减为 6 条，`core-api-0.29-v1` 由 14 条减为 7 条。升级顺序同上：先在 0.28.x 上预览、应用并运行项目测试，再升级 CLI。Struct `.contains?` 与 Enum `.contains?` 在 0.29.0 仍可用，计划在 0.30.0 随 `Contains` trait 的去留一起处理；新代码改用 `.contains-field?` / `.contains-index?`。

### 0.29.0 已删除：函数 `some?`、`join-str`、`add-watch`、`cpu-time` 与 String `.count`

下面的旧入口在 0.26.0 至 0.28.0 正式版中已有首选写法；corokia、calcit-graphviz、calcit.std 合并迁移后，已知活跃下游默认分支的源码与附带测试不再调用，因此在 0.29.0 删除。首选入口现在直接拥有原实现，行为与 0.28 的旧名相同。

| 已删除 | 首选替代 | 迁移 |
|---|---|---|
| 函数 `some?` | `non-nil?`；Option variant 用 `.some?` | 使用 0.28.x 的 CLI：`calcit calcit.cirru fix --rule core-non-nil-predicate-v1 --format edn`；也可人工改写 |
| 函数 `join-str` | `join-string` | 人工改写；`core-function-alias-v1` 只在 0.29.0 预发布版中覆盖它 |
| 函数 `add-watch` | `add-watch!` | 人工改写，参数顺序与返回值不变 |
| 函数 `cpu-time` | `monotonic-time-ms` | 人工改写 |
| String `.count` | `.len`，同样按 Unicode 标量计数 | 使用 0.28.x 的 CLI：`calcit calcit.cirru fix --rule core-collection-len-v1 --format edn`；紧凑写法 `text.count` 需人工改写 |

残留的函数调用在检查时报告未知名字，告警末尾给出首选写法，例如 `` `some?` was removed from calcit.core in 0.29.0, use `non-nil?` ``；残留的 String `.count` 得到 `E_RETIRED_METHOD`。函数 `count`、`&trait-call Countable :count` 与 `where T: Countable` 的泛型调用仍接受 String；List、Map、Set、Struct 与 Enum 的 `.count` 不受影响。宏 `with-cpu-time` 保留原名，展开后改为调用 `monotonic-time-ms`。

`core-non-nil-predicate-v1` 随 `some?` 整条退役：0.29.0 的 CLI 用 `--rule` 指定它时会提示改用 0.28.x 的 CLI。`core-api-0.28-v1` preset 由 6 条规则减为 5 条，`core-api-0.29-v1` 由 7 条减为 6 条。`core-function-alias-v1` 继续迁移 `optionally`、`join` 与 `vals`；`core-collection-len-v1` 继续迁移 List、Map、Set 的 `.count`。升级顺序同上：先在 0.28.x 上预览、应用并运行项目测试，再升级 CLI。

### 仍可用的兼容名

core 中带 `:deprecated` 标记的 26 个兼容名在 0.29.0 仍可调用，行为与 0.28 相同。新代码使用右侧的首选写法；有 fix 规则的项目先预览再应用，其余按首选写法逐处改写。

| 兼容名 | 首选写法 | 迁移 |
|---|---|---|
| `%some` / `%none` / `%ok` / `%err` | `Option :some x` / `Option :none` / `Result :ok x` / `Result :err e` | `calcit calcit.cirru fix --rule core-nominal-constructor-v1 --format edn` |
| `optionally` | `nil->option` | `calcit calcit.cirru fix --rule core-function-alias-v1 --format edn` |
| `join` | `intersperse` | 同上 |
| `vals` | `distinct-values` | 同上 |
| `section-by` | `chunks`（按固定长度切段，语义与 Rust `chunks(n)` 一致） | 同上 |
| `merge-dynamic` | `merge`，泛型值类型绑定为 `Dynamic` | 同上 |
| `concat-dynamic` | `concat`，泛型元素类型绑定为 `Dynamic` | 同上 |
| `turn-string` | `to-string` | 参数已证明为标量时用 `core-identity-conversion-v1`，其余人工改写 |
| `turn-str` | `to-string` | 人工改写 |
| `remove-watch` | `remove-watch!` | 人工改写 |
| `case-default` | `match`，默认值写成末尾 `_` 分支 | `case-default-to-match-v1`（字面量模式） |
| `foldl-shortcut` | `fold-while`，reducer 返回 `ControlFlow :continue acc` 或 `ControlFlow :break value`，从未 break 时返回累积值 | 人工改写：`(:: false acc)` 改为 `ControlFlow :continue acc`，`(:: true v)` 改为 `ControlFlow :break v`；旧写法在从未 break 时返回第三个参数 `default`，需要时在调用处判断后显式给出 |
| `some-in?` | `option:some? $ get-in x path`；只需判断路径是否存在时用 `contains-in?`（两者对值为 `nil` 的路径结果不同） | 人工改写 |
| `range-bothway` | 显式写 `range`：`range-bothway n` 写成 `range (inc (negate n)) n`，`range-bothway a b` 写成 `range (inc (- (+ a a) b)) b` | 人工改写 |
| `let{}` | `let` 逐个绑定 `&map:get m :key`；struct 值用 `.-field` | 人工改写（生态中 2 处） |
| `let-destruct` | symbol 模式写 `let`，`([] ...)` 模式写 `let[]` | 人工改写（生态中未见调用） |
| `option:let` | 嵌套 `.and-then`：`option:let ((a x) (b (f a))) body` 写成 `x .and-then $ fn (a) $ (f a) .and-then $ fn (b) body` | 人工改写（生态中 1 处） |
| `let-sugar` | symbol 绑定写 `let`，`([] ...)` 模式写 `let[]` | `let-sugar-to-let-v1`（`({} ...)` 模式人工改写） |
| 宏 `w-log` | `dbg`，打印同样的源码与值并返回值 | `calcit calcit.cirru fix --rule core-macro-alias-v1 --format edn` |
| 宏 `wo-log` / `wo-js-log` | 去掉外层，直接写参数 | 同上 |
| 宏 `flipped` | 按实际顺序写参数：`flipped f a b` 写成 `f b a` | 同上；位于 `->` 等线程宏中时需要人工改写 |
| 宏 `w-js-log` | `dbg`；需要在浏览器控制台查看宿主对象时显式写 `js/console.log` | 同上，只给 `requires-review` |

上表名字、函数 `round?`、前缀 `reduce`，以及方法 List `.add`、List/Map/Set `.count`、FfiTask `.cancel` / `.cancel-with` 与 Struct/Enum `.contains?` 按同一节奏退场：已知活跃下游默认分支（源码、附带测试/示例、宏生成代码与 CI/文档引用）清零后，在下一个非 patch 版本删除，并在本文列出删除项。

### Ref watcher 重复注册

`add-watch!` 的 Tag key 在同一个 Ref 上不可重复。JS runtime 现在与 native 一样拒绝重复注册，保留原 watcher，不再静默替换回调。确需替换已注册的回调时，先用 `remove-watch!` 移除该 key，再用 `add-watch!` 注册新回调；这两步仍是显式效果，不提供自动改写。首次注册和移除仍返回 Unit，callback 的新值、旧值参数顺序不变。此修复不扩展 WASM/WASI 的局部 Ref 支持。

### Ref 构造名

`Ref<T>` 的首选构造名是 `ref`（局部）与 `defref`（命名空间级），与 `type-of` 返回的 `:ref` 和谓词 `ref?` 一致。`atom` / `defatom` 在 0.29.0 仍可调用，读取为同一个实现，行为、类型与 native/JS/WASM 支持范围都不变。它们暂不带 `:deprecated` 标记：Respo、memof、js-ffi 等仍有 legacy `analyze quality` baseline 的项目大量使用旧名，加标记会直接改变其 `deprecatedCalls` 预算。

```bash
calcit calcit.cirru fix --rule core-ref-constructor-v1 --include-attached --format edn
calcit calcit.cirru fix --preset core-api-0.29-v1 --include-attached --format edn
```

`core-api-0.29-v1` 包含 `core-api-0.28-v1` 的全部规则并追加本规则。0.29.0 起读取器把 `ref` 解析为内建构造（与 `atom` 相同），名为 `ref` 的局部绑定或函数参数会在预处理时报错，需要改用其他名字；calcit.core 的 `add-watch!` / `remove-watch!` 参数已相应改名。`atom` / `defatom` 按与上表相同的条件退场。

### 类型表示的历史变体盘点（#1554）

`CalcitTypeAnnotation` 的 `Optional`、`Custom`、`DynFn`、`StructValue` / `EnumValue` 与 `StructDef` / `EnumDef` 在 0.30.0 仍然保留。它们不只是 Snapshot 的旧写法：当前源码、core 内置 proc 签名和活跃下游默认分支都还在构造它们，删除会让这些项目的 schema 失效，因此没有满足退场条件。

| 变体 | 当前仍在使用的位置 | 结论 |
|---|---|---|
| `Optional` | 本仓库 `src` 中 105 处 `CalcitTypeAnnotation::Optional` 引用（含 `proc_name.rs` 中返回可空值的内置 proc 签名）；`calcit/test-struct.cirru`、`test-generics.cirru`、`test.cirru` 中 19 处 `'Optional` schema；下游 Respo/alerts.calcit、Memkits/genai.calcit、Erigeron/edn-renderer、TopixIM/diary 的默认分支 | 保留。公共 schema 已由 `E_LEGACY_OPTIONAL_SCHEMA` 在严格模式拒绝，core `&` 原语与 `nil->option` 桥是内部边界 |
| `Custom` | `'Struct`、`'StructDef`、`'EnumDef`、`'Trait`、`'Impl` 作为 schema 标记，在 110 个下游仓库的 `*.cirru` 中出现（`'Trait` 440 处、`'StructDef` 434 处）；`Calcit::Impl` 的推断结果与 `from_calcit` 的兜底也使用它 | 保留。需要先为这些标记提供专用变体或明确诊断，再删除兜底 |
| `DynFn` | 19 个下游仓库的 `calcit.cirru` 在 `:args` 等位置写裸 `'Fn`（87 行，含 Respo/respo.calcit、Respo/alerts.calcit、mvc-works/ws-edn.calcit、Termina/termina）；内置 proc 与 `E_FFI_IR_CALLBACK_TYPE` 诊断也依赖它 | 保留。是否表示为参数与返回均为 Dynamic 的 `Fn`，须先让证明与诊断对两者保持一致 |
| `StructValue` / `EnumValue` 与 `StructDef` / `EnumDef` | 前者描述值的推断类型，后者描述 `defstruct` / `defenum` 产生的一等定义值，分支语义不同 | 保留，没有可合并的重复分支 |

退场条件与 [兼容入口的退场节奏](#兼容入口的退场节奏) 相同：已知活跃下游默认分支（源码、附带测试/示例、宏生成代码、CI/文档引用）对旧写法清零，依赖模块与未合并迁移核对完毕，才在下一个非 patch 版本删除，并在本文列出删除项与迁移命令。扫描范围为 calcit-lang、Respo、Cumulo、TopixIM、mvc-works、Quamolit、Cirru、Memkits、Phlox-GL、Quatrefoil-GL、Triadica、Erigeron、Termina、WebGPU-Art、worktools 等组织下带 `calcit.cirru` 的默认分支，只统计 `*.cirru` 源码。

## WASM 的 nil 类型证据

WASM 后端现在依据静态类型证据 lowering `nil?`，从而让 `non-nil? false`、`non-nil? 0` 与 native、JavaScript 保持一致。当前 scalar ABI 中 nil、false 与数值 0 的位表示不能单靠运行时比较区分；因此具体类型的参数会直接得到确定的 nil 判断结果，同时原表达式仍严格求值一次，不会跳过副作用。

泛型 helper 在直接调用点取得具体实参类型后会被单态化，跨 namespace 的普通 Calcit 引用同样有效；不需要改成 native call，也不需要为 helper 建立额外加载路径。名义 `Option` 自身不是 nil，所以 `non-nil? $ Option :none` 仍为 true，不能替代 Option 的 `.some?`。

无法取得具体类型证据的开放 `Dynamic`、未绑定泛型，以及把 nil-sensitive 泛型 helper 作为一等函数或公开 WASM 导出的边界，会明确报出 `E_WASM_NIL_TYPE_EVIDENCE`。带 `&` 的 spread 调用没有固定实参形状，不能充当泛型特化点；如果目标 helper 依赖 nil 类型证据，同样会在编译期拒绝，而不是留下运行时 trap。应改为类型已具体化的普通直接调用，或增加一个签名闭合、无需猜测 payload 类型的 wrapper。

旧 `Optional<T>` 只有在 payload 使用确定非零的引用/句柄表示时才可做运行时 nil 判断，例如 WASI `get-env` 的 `Optional<String>`；`Optional<Number>` 与 `Optional<Bool>` 仍会拒绝，因为 0/false 和 nil 在 scalar ABI 中相同。这类边界应先收紧 schema、迁到名义 `Option`，或在 Calcit 调用点完成具体化；编译器不会猜测零值，也不会为了兼容扩展动态追踪规则。此次修复不改变 scalar ABI，也没有提供掩盖开放类型的自动转换。

```cirru
assert= true $ non-nil? false

assert= true $ non-nil? 0

assert= false $ non-nil? nil

assert= true $ non-nil? $ Option :none
```

## 非 nil 谓词改名

使用 `non-nil?` 表示“值不为 nil”。旧 `some?` 已在 0.29.0 删除，它从未表示 Option variant 判断。`Option :none` 本身是一个非 nil 的名义值，因此 `non-nil? $ Option :none` 仍返回 true；要判断 Option variant，请使用 `.some?`、`.none?` 或原生 `match`。

```cirru
assert= true $ non-nil? false

assert= true $ non-nil? 0

assert= false $ non-nil? nil

assert= true $ non-nil? $ Option :none

assert= false $ (Option :none) .some?
```

0.28.x 的 CLI 提供迁移规则，需在升级 CLI 前运行：

```bash
calcit calcit.cirru fix --rule core-non-nil-predicate-v1 --format edn
```

规则只改写编译器已解析到 `calcit.core/some?` 的源码引用，并使用明确限定的 `calcit.core/non-nil?` 防止同名局部变量或 import 改变解析结果；它不把 `some? option` 猜成 `option.some?`，不改写用户自定义同名函数。0.29.0 的 CLI 不再提供该规则，残留调用由未知名字告警定位。

`non-nil?` 仍不能擦除宿主空值边界：`JsNullish<T>` 必须使用 `js-present?` / `js-nullish?`。对名义 Option 使用 `non-nil?` 也会保留提示，因为它通常表示把“容器存在”误当成“variant 为 some”。

## 字符串搜索索引单位修复

`.find-index` / `str-find-index` 现在返回 Unicode 标量索引，与 `.get/.slice/.len` 一致。此前 native/WASM 返回 UTF-8 字节偏移，JS 返回 UTF-16 单元偏移，例如在 `😀a` 中搜索 `a` 分别得到 4 和 2；修复后统一为 `Option :some 1`。ASCII、找不到、空 pattern 与首次匹配行为不变，`Option<Number>` 类型也不变。

正常源码无需改写；请移除应用中为旧偏移错误添加的编码补偿，并检查把搜索结果交给 JS `slice` 或协议 byte offset 的 FFI 边界。此类补偿含业务语义，不提供全局自动 fix；不能把返回值直接当作宿主编码单位。需要 UTF-8 长度时继续显式使用 `&str:utf8-byte-count`，不要用 `.len` 代替。调用形态、组合字符与跨目标范围见 [String 搜索契约](../data/string.md#子串搜索索引)。

## 开放值进入具体参数

严格模式下，显式开放的值进入具体参数前需要证明。显式开放的值指由 `hint-fn` 或 schema 声明为
`Dynamic`、`List<Dynamic>`、`Map<K,Dynamic>` 等开放类型的函数参数，以及用 `&let`/`let` 从这些参数计算出的开放值，
例如从 `List<Dynamic>` 参数中取出的元素；具体参数包括带 `Number`、`String`、`List<T>` 等合同的函数参数（含
`+`、`inc` 等 core 函数）和闭合 Enum 的 payload，例如 `Data :number`。缺少证明时在调用点报告
`E_CALL_ARGUMENT_UNPROVEN`，位置指向该实参，而不是被调函数内部的运算。下例中开放元素直接进入 Number 参数：

```cirru.no-check
let
    number-only $ fn (value)
      hint-fn $ {} (:args ([] 'Number)) (:return 'Number)
      &+ value 1
    from-open $ fn (items)
      hint-fn $ {} (:args ([] (:: 'List 'Dynamic))) (:return 'Number)
      number-only $ &list:nth items 0
  from-open $ [] |wrong
```

迁移时在调用前给出证明：用 `number?` 等谓词收窄、`match (data-view v)` 分类，或用 `try-decode-map-as`
解码；无法处理的分支由业务决定返回值或报错。

```cirru
let
    number-only $ fn (value)
      hint-fn $ {} (:args ([] 'Number)) (:return 'Number)
      &+ value 1
    from-open $ fn (items)
      hint-fn $ {} (:args ([] (:: 'List 'Dynamic))) (:return 'Number)
      &let
        v $ &list:nth items 0
        if (number? v) (number-only v) 0
  assert= 3 $ from-open $ [] 2
```

开放值仍可直接保存、转交给 `Dynamic` 参数或经泛型函数传递；未标注且没有类型证据的局部变量不受这条规则影响。

### 限制

- 只检查调用点可见的类型证据，不在运行时插入入口检查。
- 内建 proc（如 `&+`）的参数仍沿用原有检查。
- 类型推导暂时无法表达而回退为 `Dynamic` 的值（Map 条目 pair、匿名 enum payload、未声明合同的回调参数等）不在本规则内。

## Map 条目参与排序与组件调用

旧的 `&map:to-list` 生成异构的 `[key value]` 列表；即使输入是 `Map<K,V>`，
该列表的成员也不能同时被推断为 `K` 和 `V`。把它解构后直接传给接受具名 Struct 的函数，
可能遇到 `E_DYNAMIC_NOMINAL_ARGUMENT`。此处不应给旧原语伪造一个同质列表类型，也不要用
`assert-type` 将未经证明的动态值硬转为具名类型。

需要保留类型时，改用 `map-entries`。它返回 `List<MapEntry<K,V>>`，条目提供 `:key` 与
`:value` 字段；下游排序、`map-indexed` 与组件调用继续按各自的类型检查。以下片段依赖
项目中的 `tasks`、`comp-task` 定义，是迁移模板而非独立可执行的表达式：

```cirru.no-check
-> tasks map-entries
  &list:sort-by :key
  map-indexed $ fn (idx entry)
    comp-task (:value entry) idx false || ||
```

如果排序键位于值内部，例如 `Task.sort-id`，仍可对条目使用 `sort`，但当前对嵌套的比较函数
不会自动从泛型容器推断 `a`、`b` 的精确类型；在比较函数内声明 `hint-fn` 的 `:args` 为
`MapEntry<K,V>`，再读取 `(:sort-id $ :value a)`。例如，`Task` 是项目中的具名 Struct 时：

```cirru.no-check
sort entries $ fn (a b)
  hint-fn $ {}
    :args $ [] (:: 'MapEntry 'String 'app.schema/Task) (:: 'MapEntry 'String 'app.schema/Task)
    :return 'Number
  &compare (:sort-id $ :value a) (:sort-id $ :value b)
```

这是局部、显式的类型补充，不需要新增
编译器特殊规则。`map-entries` 不会自动修改旧调用；升级时应在实际消费者项目中运行
`analyze check-public`、对应测试及 JS 构建，确认业务排序和渲染结果。

## 0.19 兼容清理与 CLI 入口收敛

`%{}?` 与底层 `&%{}?` 已退役，`--compat-types` 也不再恢复其隐式 `nil` 补字段行为。
旧代码应改用完整的 `%{}` 构造；确实可能缺失的字段先声明为 `Option<T>`，再显式提供
`Option :none`，不能由编译器猜测业务默认值。旧调用会给出 `E_PARTIAL_STRUCT_NIL_FILL` 迁移错误。

基于 String path 的 `try-read-file path` 与 `try-write-file path content` 包装已移除。
分别改为 `.read-text (fs:path path)` 与 `.write-text! (fs:path path) content`，仍返回相同的
`Result<String,String>` 与 `Result<Unit,String>`；不要迁回会抛出异常的原始 `read-file` /
`write-file`。`try-read-dir` 暂留供递归 `.walk-dir` 使用，后续单独整理。

entry 的 `:target` 现在可以通过既有命令族撤销：`calcit config unset target [--entry <name>]`
删除该 entry 的 `:target`，恢复“未指定 target”的语义（`config show` 显示 `(none)`）。
重复执行 unset 稳定返回 “already unset”，且不改写 Snapshot 内容；`unset` 目前只接受 `target`，
其他 key 会给出明确错误。同时用于 browser 与 Node 的项目不必再手改 Snapshot 回退。

JavaScript runtime 的 `to-js-data value true/false` 布尔第二参数已退役；需要保留 tag 键的冒号时，
改用 `to-js-data value $ {} (:add-colon true)`，无需该行为时只传 `value`。旧布尔写法现在明确报错，
避免在升级后静默转换出不同的 JSON key。`@calcit/procs` 的旧导出
`_$n_enum_def_$o_has_variant` 也已移除；直接调用 runtime 的代码应使用与
`&enum-def:has-variant?` 对应的 `_$n_enum_def_$o_has_variant_$q_`。

当前 Snapshot 的命名空间代码必须使用 `ns name`；旧式 `:ns name` 已被拒绝，并会给出迁移提示。
升级旧快照时，可对没有 import 的命名空间执行
`calcit calcit.cirru edit imports <namespace> --input-format cirru --code 'quote $ []'`；
这会通过结构化编辑重建 `ns` 节点。已有 import 的命名空间应先查询并保留规则，再用
`edit imports` 提交，不能用空列表覆盖。
`%Expr` / `%Leaf` 是早期快照的展示结构；当前诊断只对 `quote` 代码做 Cirru 预览。
若旧快照仍使用这两种结构，请先用其对应旧版工具迁移，不要把它们当作当前语言代码节点。
`docs check-md` 读取项目依赖时也使用同一严格 Snapshot loader，不再单独从旧格式中提取模块列表；
先完成快照迁移，再运行文档代码块检查。
普通 `deftrait` / `defimpl` 方法键原来的 `:render` 等 tag 写法已退役，改成 `.render` 等 dot 写法；
类型标注中的 `:fn` 不变。external-object trait 的属性条目（如 `:field 'String`）仍使用 tag，
不要把它们批量改为 dot。

顶层 `calcit libs`（或旧二进制名 `cr libs`）已移除，统一改为 `calcit docs remote-libs`。
子命令和参数保留：`libs search <keyword>`、`libs readme <package> [--file <file>]`、
`libs scan-md <module>` 分别改为 `docs remote-libs search`、`docs remote-libs readme`、
`docs remote-libs scan-md`；不带子命令的列表查询改为 `calcit docs remote-libs`。
升级时请同步修改项目脚本和 Agent 指南中的旧命令。查询已安装模块的文档时，优先使用
`calcit docs list/read/search --module <module>`，无需经过远端类库索引。

`calcit exec` 也合并进现有 `eval` 命令族：原来的 `echo 'range 10' | calcit exec`
改为 `echo 'range 10' | calcit eval --stdin`。`--dep` 仍可重复传入，stdin 与位置参数片段互斥；
未指定位置参数或 `--stdin` 时直接报错；显式传 `--stdin` 后会等待输入直到 EOF，空输入也会报错。

对于编译器能够证明等价的一对一迁移，先运行 `calcit calcit.cirru fix --preset surface-latest-v2 --format edn`
审阅结构化计划，再用
`--apply --expect-revision <revision>` 原子应用；完整安全边界见 [Compiler-guided Source Fixes](fix.md)。

若起点是仍依赖 0.14.15 旧修复规则的项目，请先按
[历史两阶段桥接步骤](upgrade-history.md#01415-的两阶段源码修复桥接)完成旧版迁移，
再使用当前 preset；不要把旧 rule ID 交给当前版本。

升级或类型迁移后，应显式预览一次问题写法修复。`fix` 的默认 preview 本身就是检测入口，不写 Snapshot；其中
`redundant-do-v1` 会定位 `defn`、`fn`、`let` 等多表达式 body 中不必要的 `do`，并返回可审阅的 source path、
fingerprint 和 splice replacement；`single-expression-do-v1` 会解包普通可执行位置中只有一个 payload 的 `do`：

```bash
calcit calcit.cirru fix --preset surface-latest-v2 --format edn
```

不要用全文搜索后批量删除 `do`：`if` 分支、调用参数和 binding value 等单表达式位置仍需要它来组合多个步骤。
若 preview 返回建议，确认 `:data :validation :status` 为 `passed` 后，再复制报告中的 `:revision` 执行带保护的 apply；
若建议为空，则接受 `not-needed` 并跳过 apply。完整示例见
[检测并修复冗余 `do`](fix.md#检测并修复冗余-do)。这是一项显式源码整理，不会增加普通编译 warning。

## 当前目标与历史版本迁移

当前项目以默认严格 `--check-only`、对应 entry 的实际运行和目标后端回归为升级验收；
静态分析报告用于定位，不另设覆盖率或 Dynamic 数量门槛。WASM 请选择 `calcit wasm` 或
`calcit wasi`；`calcit wasi` 从 0.24 起默认输出 WASI 0.3.1 `wasi:cli/command` Component，
Preview 1 core module 需显式 `--boundary native`（时钟、`wait-ms`、安全随机数、`.read-dir`
等尚未迁移能力仍走该路径）。Component 的当前支持范围以
[WASM Component 边界](../installation/wasm-component-boundary.md) 为准。

0.13–0.15 的严格诊断桥接、`cr-wasm` 迁移及 Component ABI 演进步骤已移至
[历史版本迁移记录](upgrade-history.md)。历史版本当时的“尚未支持”不代表当前能力。

---

## 1）升级前检查位置

### 先建立可回滚基线

不要在未提交的业务修改上直接同时升级 CLI、依赖、Snapshot 和类型。先确认工作区状态，创建升级分支，
并记录旧工具链下确实成功的命令、entry 和构建产物：

```bash
git status --short
git switch -c upgrade/calcit-latest
calcit --version
calcit compact.cirru            # 仅在 0.13.48 或更早的旧工具链记录基线
yarn test                       # 替换为项目原有的测试/构建命令
```

如果旧项目当前就不能通过原有测试，应先把失败记录为已知基线；不要把它误归因于新版类型系统。
Snapshot 文件迁移、依赖升级和类型修复建议分别提交，任何阶段都能单独回退和比较。
不要要求旧 CLI 支持本文后续介绍的所有新版子命令；这些校验应在 Step A 更新工具后再执行。

升级前先检查以下文件与配置是否齐全：

- 运行入口：`calcit.cirru`（严格版本不再接受 `compact.cirru`）
  - `:entries.default`（默认入口及 `:mode`）
  - `:entries.<name>`（额外入口及各自 `:mode`）
- 命令入口：`README`、项目脚本、CI workflow
- Node 工具链：`package.json`、`yarn.lock`、Corepack/Yarn 版本
- 注意 git fetch 检查最新历史, 避免基于老版本操作导致变更冲突
- 依赖边界：运行/编译期需要的模块放在 `:dependencies`；只供当前项目测试、examples、文档检查和维护脚本使用的模块放在 `:dev-dependencies`
- 结构化编辑优先使用 `calcit edit` / `calcit tree`；若直接改过 `calcit.cirru`，提交前执行一次 `calcit calcit.cirru edit format`
- 静态质量基线：`check-types`、`weak-types`、公开 namespace examples 与 Markdown 示例

### 快照文件迁移说明

以前的 Calcit 项目使用两套文件：

- `calcit.cirru` — 存放完整 AST 快照，内容包含全部编译信息（带所有代码位置、类型标注等）
- `compact.cirru` — 存放精简代码，是人工读写的主要文件

Calcit 0.13.48 是接受 `compact.cirru` 文件名的最后一个兼容版本；0.13.49 起会在反序列化前拒绝该文件名并给出迁移命令。
严格版本只把精简 Snapshot 保存在 `calcit.cirru` 中。
如果两个文件同时存在，不要仅凭文件名猜测哪个是有效源码；先在旧版本下分别检查 Git 历史、文件体积、
项目脚本和实际运行入口。确认 `compact.cirru` 是当前可运行的精简 Snapshot 后再迁移：

1. 确认 `compact.cirru` 是项目实际精简化代码，`calcit.cirru` 是完整 AST 快照（差异通常很大）
2. 使用 0.13.48 或更早的可运行工具链确认基线，再在独立提交或临时分支中将 `compact.cirru` 复制/重命名为 `calcit.cirru`，不要和业务逻辑修复混在一起
3. 执行 `calcit calcit.cirru edit format`，审阅 diff，再对新的 `calcit.cirru` 运行 `--check-only` 和原有 entry/构建测试
4. 新旧入口行为一致后再删除旧文件并提交；后续所有 `calcit` 命令都显式基于单一的 `calcit.cirru`

如果新版 `calcit` 连旧的完整 `calcit.cirru` 都无法反序列化，先确认旁边的 `compact.cirru` 能在旧工具链运行，
并从 Git 状态/历史确认它是最后的有效精简源码。当前 `edit format` 有一个与 runtime loader 隔离的一次性迁移入口，
可读取早期 compact Snapshot 的 direct quoted namespace/definition 以及顶层 `:configs`，再写成 canonical Snapshot；
普通运行、检查和其他编辑命令仍严格拒绝这些旧结构。按以下可恢复步骤重建：

```bash
cp calcit.cirru calcit.full-snapshot.backup.cirru
cp compact.cirru compact.migration-backup.cirru
$EDITOR compact.cirru
# 在每个 ns 规则中将 :require-macros 改为 :require，并确认不再有旧 clause
if rg -q ':require-macros' compact.cirru; then
  echo '请先逐个迁移 compact.cirru 中的 :require-macros 规则'
  exit 1
else
  status=$?
  if [ "$status" -ne 1 ]; then
    echo '无法检查 compact.cirru 中的 :require-macros 规则'
    exit "$status"
  fi
fi
cp compact.cirru calcit.cirru
calcit calcit.cirru edit format
git diff -- calcit.cirru
calcit calcit.cirru --check-only
```

若 `rg` 没有匹配，才继续复制和格式化；如果仍有匹配，应逐个编辑 namespace 规则，而不是用全局替换，
以免改动代码字符串中的同名文本。不要删除备份或旧文件，直到所有 entry 与原有 native/JS 测试均通过。现在反序列化错误会带上失败的
Snapshot 路径；如果同目录存在 `compact.cirru`，还会直接给出上述恢复方向。严格版本不能直接格式化名为
`compact.cirru` 的文件；必须先复制或重命名为 `calcit.cirru`。如果迁移日志报告 direct quote/configs 计数，需单独审阅生成的
`CodeEntry` / `NsEntry` 和 `entries.default`。旧普通 definition 会得到明确的 `Dynamic` schema；direct-quote `defmacro`
会按参数列表恢复为保守的严格 `Macro` contract：每个参数为 `Syntax`，expansion 为 `Expr<Dynamic>`，capabilities 为空。
这只让迁移结果可被当前严格 loader 读取，不会猜测业务类型或授权编译期副作用；随后应人工收窄语义类型并声明确需的 capability。

旧 namespace 里的 `:require-macros` 也必须在 Snapshot 规范化前处理。宏与普通值现在共用
`:require` 规则，例如把：

```cirru.no-check
ns app.main $ :require-macros
  legacy.macros :refer $ defcomp
```

改为：

```cirru.no-check
ns app.main $ :require
  legacy.macros :refer $ defcomp
```

遇到旧写法时，加载阶段会明确报告 `:require-macros` 迁移提示，而不是只返回笼统的 invalid `ns` form。

> `calcit` 仍然读取 Snapshot，只是当前约定把精简 Snapshot 直接命名为 `calcit.cirru`；不要把它理解为
> “不再依赖 Snapshot”。`calcit edit` / `calcit tree` 会直接修改该文件。确认迁移完成后，可以移除旧的
> `.gitattributes` generated 标记和过时的双文件生成脚本；不要把仍可能承载源码的文件直接加入 ignore。

新版 CLI 在任何 Snapshot 写入前都会核对相邻 `deps.cirru :calcit-version`。这避免全局安装的新版本
把 Snapshot 写成项目 CI 固定的旧版本无法读取的格式。版本不一致时，先决定是使用项目固定版本完成
当前编辑，还是显式运行 `caps deps.cirru upgrade --all` 升级整条工具链；不要为了绕过门禁临时删除版本声明。

---

## 2）标准升级流程（建议顺序）

下面流程按“先确认版本，再对齐工具链，再更新依赖，最后按 CI 链路验证”的顺序执行。

### 快速命令清单

使用页首 [当前升级闭环](#当前升级闭环) 的唯一命令清单；下文 Step A–E 展开其中第 3–4 步，Step F 展开第 8–10 步。
项目自己的 test/build 命令从 `package.json` 和 CI 读取，Vite 或某种静态分析报告不是所有项目的默认升级门禁。

旧项目遇到 `E_LEGACY_OPTIONAL_PARAM` 时，可先按单个定义检查原始 `?` 参数：

```bash
calcit calcit.cirru fix --rule optional-parameters-v1 --ns app.main --def legacy-helper --format edn
```

预览中的 `declared_type` 来自现有 schema；仅在声明足够具体时才给出 `candidate_type`。
如果所有尾参数都具备可用的声明类型，且原 Fn 声明的参数、返回与 rest 没有开放的 `Dynamic` 成员，
`candidate_fn_schema_edn` 会给出完整的只读 Fn schema 候选，
保留原参数、返回、泛型与 feature 元数据，并把旧 `?` 尾参数写成 `Option<T>`。它是 Cirru EDN 字符串，
可供审阅或作为手动编辑的起点；不表示函数体与调用点已可安全自动改写。
命名类型引用需要能解析为确定的 nominal struct/enum；无法证实的别名不生成完整候选。
同一建议的 `origin_chain` 还列出编译器解析到的项目源码引用：直接调用、带展开参数的调用、
函数值引用及 macro 生成的引用分开报告，并标明直接调用的实参数量，以及显式 `nil`、`false` 的位置。
`project-reference-scan` 明确报告扫描范围和无法预处理的定义；目前只覆盖定义源码，
definition-attached tests/examples 与仓库外消费者仍未纳入完整性证明。
候选并不证明函数体对缺失值的处理、显式 `nil` 与省略调用是否等价，也不证明 macro、跨模块消费者
和函数值调用均可定位。因此该规则目前一律标记 `needs-review`，不接受 `--apply`，也不在默认 preset 中。
人工迁移应在审阅这些语义后，把声明和所有调用点一起改为明确的 `Option` 契约并运行严格检查与业务测试；
不要把 `nil` 一律改成 `Option :none`，也不要为了通过检查扩大 `Dynamic`。

### Step A：确认 Calcit CLI 版本

```bash
calcit --version
caps --version
caps --help
```

说明：`calcit` 和 `caps` 是独立发布的用户工具。安装或升级前先读取项目 `deps.cirru :calcit-version`，
确认对应 release 已发布，再分别固定 Calcit 与经过真实项目 smoke 验证的 caps 稳定版本；
旧版安装组合仅见[历史版本迁移记录](upgrade-history.md)。不要先用未经验证的旧 `caps` 改依赖，
再用新 `calcit` 判断结果；也不要只更新本机而让 CI 继续安装另一版本。若团队通过其他受控方式分发二进制，
使用该方式即可，但要分别记录实际版本，并确认 `caps --help` 已包含项目需要的新选项。
从对应 release notes 或 `setup-calcit` 已验证的版本矩阵选择一对明确版本；
本手册不固定会过期的全局安装版本，也不要省略安装命令的 `--version` 而隐式安装两个 latest。

> ⚠️ CI 中 Calcit runtime/compiler 的项目版本来自 `deps.cirru :calcit-version`；caps 使用 setup-calcit 独立固定的稳定版本，必要时通过 `caps-version` 显式覆盖。普通 workflow 不重复传 `version`。Action 会对新 Calcit release 临时提供 `cr -> calcit` 兼容链接；对旧 release 则回退到 `cr` asset 并暴露 `calcit`。新命令统一写 `calcit`。已发布的 `calcit-lang/setup-cr` tag 继续支持旧项目；GitHub Actions 不会为 Action 仓库改名重定向，因此迁移必须显式替换 `uses:`。详见 [GitHub Actions](../installation/github-actions.md)。

### Step B：先对齐项目版本与 Node 工具链

重点先检查并对齐以下几处：

- `deps.cirru` 里的 `:calcit-version`
- `package.json` 里的 `@calcit/procs`
- `package.json` 里的 `packageManager`
- `.yarnrc.yml` 是否需要 `nodeLinker: node-modules`
- `.gitignore` 是否已忽略 `.yarn/*.gz`（避免 Yarn 压缩状态文件入库）

先把这些基础版本与工具链约定对齐，再继续更新依赖，能减少后面重复改 lockfile 或 CI 的次数。

非常旧的项目如果仍使用 `package.cirru`，先把它重命名为 `deps.cirru` 并单独提交；新版 `caps`
会拒绝旧文件名并给出迁移提示。不要同时保留两份依赖清单，否则项目脚本和维护者可能更新不同文件。

更新依赖前先读取当前 Agent/CLI 指南，避免沿用旧命令边界：

```bash
calcit docs agents --contract
```

### Step C：检查并更新 `deps.cirru`

先审计依赖分组。新版 `caps` 对根项目同时安装 `:dependencies` 和
`:dev-dependencies`，但递归解析某个依赖模块时只读取它的 `:dependencies`，不会把该模块
自己的开发依赖带入消费者。升级旧项目时，应把测试、examples、文档验证与维护工具专用模块
迁到 `:dev-dependencies`，避免递归依赖图继续无边界扩张：

以下仅展示分组结构；尖括号版本占位符必须换成已发布并经项目验证的具体版本：

```cirru.no-check
{} (:calcit-version |<published-calcit-version>)
  :dependencies $ {} (|calcit-lang/respo.calcit |<respo-version>)
  :dev-dependencies $ {} (|calcit-lang/calcit-test |<test-version>)
```

可以用 `caps add --dev <org/repo>@<ref>` 和 `caps remove --dev <org/repo>` 管理开发依赖。
同一个仓库不要以不同 ref 同时出现在两个分组；新版会直接拒绝这种歧义配置。

```bash
caps upgrade --all
```

说明：`caps upgrade --all` 会检查 `:dependencies` 与根项目的 `:dev-dependencies`，更新
对应分组中的依赖版本与 `:calcit-version`；如果确实发生升级，还会顺带执行一次
`yarn up @calcit/procs@<calcit-version>`，把 JS 运行时包精确同步到当前 Calcit 发布链路。

如果依赖清单本来已经是最新、但 `package.json` 中的 `@calcit/procs` 仍旧，`caps upgrade --all`
可能没有产生更新动作。此时应显式执行 `yarn up @calcit/procs@<calcit-version>`，审阅
`package.json` / `yarn.lock`，然后在 `yarn install` 后运行 `caps verify --toolchain`。这个命令会
要求 `deps.cirru :calcit-version`、PATH 中 `calcit` 报告的版本、`package.json` 的 `@calcit/procs` 声明
以及 Yarn 解析出的实际包版本全部精确一致；caps 自身独立发布的版本号不参与这一相等检查，适合直接作为 CI 门禁。

如果你只想批量把旧版本提升到最新标签，也可以继续用：

```bash
caps outdated --yes
```

这个命令只更新 `deps.cirru`，不触发 `yarn up @calcit/procs`。

### Step D：同步模块内容

```bash
caps
caps tree
caps status
caps verify
```

说明：这一步才会按当前 `deps.cirru` 下载/同步模块内容。根项目的两个依赖分组都会安装，
依赖模块的 `:dev-dependencies` 会在递归解析中排除。`tree` 用于审阅最终递归图，`status` 检查
项目链接和期望版本，`verify` 进一步检查 immutable store、源码修改和 native 构建收据。分支 ref
或版本冲突在迁移期可以先作为明确告警处理；准备固定 CI 时再用 `caps --strict --ci` 阻断这些告警。

### Step E：用 Yarn Berry 安装并校验

```bash
corepack enable
corepack prepare yarn@4.12.0 --activate  # 示例；优先采用 packageManager 固定的版本
yarn --version
yarn install --immutable
caps verify --toolchain
```

说明：团队若习惯 Yarn Berry，建议固定 `packageManager` 并使用 `--immutable` 做一致性校验。

如果项目仍依赖 `node_modules` 目录解析，还应补一个 `.yarnrc.yml`：

```yaml
nodeLinker: node-modules
```

### Step F：从 CI workflow 和 package.json 提取检查命令并本地先跑

先看 `.github/workflows/` 里实际执行了哪些命令，再看 `package.json` 里是否有额外构建脚本，然后按同顺序在本地跑一遍。

常见链路例如：

```bash
caps && yarn install --immutable
calcit calcit.cirru --check-only
calcit calcit.cirru
calcit calcit.cirru --entry <entry-name>
calcit calcit.cirru js && yarn vite build --base=./
```

`calcit calcit.cirru` 默认单次执行并选择 `entries.default`；指定 entry 时用 `--entry <name>`。entry 的 `:mode` 已决定 native 运行或 JS 生成，项目脚本与 CI 应优先依赖该配置。只有热更新才加 `-w` / `--watch`；显式 `js` 是兼容/定向 codegen 覆盖，不是每个 JS 项目的必需写法。

`--check-only` 会同时预处理所选 entry 的 init/reload 定义，因此可以发现遗留的 `:reload-fn`
已不存在、调用参数/返回值不匹配、Struct/Enum 字段错误、trait 实现不完整等问题。它在预处理产生
warning 时会非零退出，是类型逐渐严格过程中的主要编译门禁。named entry 不继承 default 配置，
所以必须逐个执行，不能只验证 default：

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test --check-only
calcit calcit.cirru --entry production --check-only
```

旧项目若一次出现大量错误，按“配置/缺失定义 → deprecated API → nominal data/trait → 函数参数和
返回值 → dynamic/nil debt”的顺序修复。每清完一类就提交并重跑所有 entry，避免用
`&core:ignore-type-warning`、`--skip-arity-check` 或批量改成 `'Dynamic` 掩盖迁移问题。

`--check-only` 的范围是所选 entry 可达的预处理路径，不等于“仓库中每个公开定义都已检查”。
未被应用入口调用的类库 API 应由 definition-attached tests、`check-examples` 或真实消费者覆盖；
不要因为 default entry 通过就跳过公开 namespace 和其他 entry。

如果 `package.json` 里有编译、构建、测试相关脚本，也应本地执行一遍；没有额外脚本可跳过。若项目直接通过 Vite 构建，可执行：

```bash
yarn up vite
yarn vite build --base=./
```

说明：若项目依赖 Vite，升级时建议显式执行一次 `yarn up vite`，并重跑构建确认兼容性。

例如还有：

```bash
yarn <script-name>
```

目标：把 CI 会跑的命令和项目脚本都在本地提前验证，减少合并后失败概率。

---

## 3）近期项目结构与类型迁移

这一节必须在 Step A–E 已完成、项目确实使用新版 `calcit` 和新版依赖后执行。先运行一次
`calcit calcit.cirru edit format` 并单独审阅/提交规范化 diff，再根据 `--check-only` 和静态报告逐类
修复语义问题；不要让自动格式化与大规模业务修复混在同一个不可审阅的提交中。

### 3.1 类型标注语法规范化

新版本把 schema 类型的推荐写法统一为 quoted symbol：`'String`、`'Number`、`'List`、`'Ref`、
`'Fn` 和 `'Dynamic`。旧的 `:string`、`:number`、`:list`、`:ref`、`:fn`、`:dynamic` 仍可加载，
以便平滑升级；普通 tag 数据（例如 enum variant `:ok`、struct field key 和 schema 的
`:return`/`:kind` key）不会被改变。

```bash
calcit calcit.cirru edit format
git diff -- calcit.cirru
calcit calcit.cirru --check-only
```

`edit format` 只改写 schema、`hint-fn`、`assert-type`、`unsafe-coerce`、`defstruct` 和 `defenum`
等类型位置，并将 entry schema 重新序列化为 canonical symbols；它不会猜测或加强实际类型契约。
提交前检查 diff，尤其是具有手写 tag 数据的宏或 DSL。

### 3.2 Struct / Enum 数据模型命名迁移

新数据模型明确区分定义和值：`defstruct` 返回 `StructDef`，`defenum` 返回 `EnumDef`；实例类型
分别是 `Struct` 和 `Enum`。没有具名定义的临时值使用 `%{} _ ...` 和 `%:: _ ...`。旧的
record / tuple 公开名称会产生 `W_REMOVED_DATA_API`，诊断中同时给出替代写法；新 Snapshot 不再
写出 `:record`、`:tuple`、`Record` 或 `Tuple`。

常用迁移如下：

| 旧写法 | 新写法 |
| --- | --- |
| `record?` | `struct?`（值）或 `struct-def?`（定义） |
| `tuple?` | `enum?`（值）或 `enum-def?`（定义） |
| `record-struct` | `struct-definition` |
| `tuple-enum` | `enum-definition` |
| `record-with` / `record-match` | `struct-with` / `struct-match` |
| `&record:*` / `&tuple:*` | 对应的 `&struct:*` / `&enum:*`；定义元数据使用 `&struct-def:*` / `&enum-def:*` |
| `%:: Result :ok value` | `Result :ok value` |
| `%{} Person (:name name)` | `Person :name name` |

具名定义优先直接调用；`%::` / `%{}` 只保留给显式运行时 prototype、动态跨模块构造和兼容边界。
`surface-latest-v2` 会自动处理能静态解析到项目定义的旧写法，并跳过匿名 `_`、动态或被局部遮蔽的原型。

Struct 字段是定义的一部分，因此已知 struct 上的 `:field` 和 `.field` 直接返回字段声明类型，
不再自动包装 `Option<T>`。不存在的字段会在静态检查阶段报告，运行期也会抛出普通错误；
升级业务代码时应删除这类访问后的 `.unwrap`。Map 等动态容器的访问仍返回 `Option<T>`，
`get-in` 也继续保留可失败路径语义，不能批量删除其 unwrap。

推荐先执行 `calcit calcit.cirru --check-only`，按诊断逐项替换，再运行完整 JS 回归。不要先全局删除
`.unwrap`；只处理接收者已被推断为 Struct 且字段在 `defstruct` 中声明的访问。

#### Option 返回 API 对照

以下 API 不再用 `nil` 或 `-1` 表示缺失。把结果直接传给算术、字符串或集合函数时，诊断会显示完整的
`Option<T>` 推断类型，并建议用类型化 `*-or` 查询终点、`.unwrap-or`、`if-let` 或 `match` 显式处理：

| API | 当前返回类型 | 迁移注意点 |
| --- | --- | --- |
| `find-index` | `Option<Number>` | 不再用 `-1`；索引运算前先处理 `Option :none` |
| `first` / `last` | `Option<T>` | 空集合和空字符串可能没有元素 |
| `nth` | `Option<T>` | 越界是 `Option :none`；不要把结果直接当元素值 |
| `get` | `Option<T>` | Map/List 等可缺失查找返回 Option；显式 `Dynamic` 的运行值若为 Struct，则按运行时字段名返回 `Option<Dynamic>`；已知 Struct 改用 `(:field value)` |
| `get-in` | `Option<T>`（开放动态路径常为 `Option<Dynamic>`） | 任一路径缺失都是 `Option :none` |
| `get-env` | `Option<String>` | 未设置的环境变量是 `Option :none` |

不要用 `str` 或 `turn-string` 掩盖尚未处理的 `Option<T>` / `Result<T,E>`。默认 strict mode 会以
`E_NOMINAL_ENUM_STRINGIFICATION` 拒绝这种隐式转换；先用 `match`、`.unwrap-or` 或对应 helper 取出 payload。
如果目的就是打印 enum 的诊断表示，显式使用 `to-lispy-string`。迁移期间可用
`--compat-types` 暂时保留旧渲染路径。

默认严格诊断还会检查这些访问 API 的接收者能力。`first`、`last`、`nth` 只接受可静态确认的
`List<T>`、`String` 或 `Enum`，`get` 另外接受 `Map<K,V>`；把 Number、Set、静态已知 Struct、函数或未收窄的
optional/FFI host value 传入时会报告 `E_UNSUPPORTED_INDEXED_RECEIVER`，而不是把 core schema 中保留的
Dynamic 接收者位置当成任意值逃生口。Struct 字段改用 `(:field value)`，optional receiver 先 match/unwrap，
FFI value 在 adapter 边界完成 validate/convert。显式声明的 `Dynamic` receiver 仍保留开放数据运行时路径；
当运行值为 Struct 时，`get` 按 tag/string/symbol 字段名返回 `Option<Dynamic>`，便于在不调用 raw primitive
的前提下实现允许缺失的边界查询。

```cirru
let
    xs $ [] 1 2 3
    predicate $ fn (x) > x 1
  hint-fn predicate $ {} (:args $ [] 'Number) (:return 'Bool)
  let
      idx $ find-index xs predicate
      safe-idx $ idx .unwrap-or 0
    &+ safe-idx 1

match (get-env |APP_MODE)
  (:some mode) (println mode)
  (:none) (println |development)
```

上例使用原生 `match`，它统一处理 `Option` 的 `:some` / `:none` 并提供穷举检查。
Calcit 0.14.16 已移除旧 `tag-match` 表层入口；升级前请用 Calcit 0.14.15 执行
`calcit fix --rule tag-match-to-match-v1`。

只有业务语义确实有合理默认值时才用 `unwrap-or`；需要区分“缺失”和“存在”时保留两个分支。

查询结果立即采用默认值时可直接机械迁移，不改变原查询 API 的 Option 契约：

```cirru.no-check
get-or config :port 6000
; =>
(get config :port) .unwrap-or 6000

get-env-or |mode |release
; =>
(get-env |mode) .unwrap-or |release
```

`get-in-or`、`first-or`、`last-or`、`nth-or` 同样改为对应查询后调用 `.unwrap-or`。
fallback 必须与 payload 类型兼容；需要区分缺失分支时改用 `if-let` 或穷尽 `match`。

上述 method 迁移要求查询接收者能静态推断为 `Option<T>`。如果 legacy Map、配置或 FFI
边界仍把查询结果擦除为 `Dynamic`，检查器会报告 `W_DYNAMIC_NOMINAL_METHOD_RECEIVER`，而不是
允许代码在运行时把 Option 值误当作 operator。先在真实边界 decode 或 narrow，并声明能由实现
证明的 schema；业务层得到 `Option<T>` 后使用接收者 method。`option:unwrap-or` 等函数形式不是
未类型化边界的逃生口：把裸 Dynamic 传入它们同样受到严格名义参数检查，不能靠具体 fallback
证明输入已经是 Option。已证明的 `Option<Dynamic>` 可以合法保留开放 payload；读取结果后，
仍须在具体使用处提供相应证据，不能把 fallback 当作 payload 验证。

开启 strict mode 后，同一问题会升级为 `E_DYNAMIC_METHOD_DISPATCH`（prefix）或
`E_DYNAMIC_POSTFIX_METHOD`（postfix），不再继续生成运行时动态派发。这里的迁移是确定性的：补出
`Option<T>` / `Result<T, E>` receiver schema，或用受检 adapter 先转换外部值，再返回名义结果；
仅改成 `option:*` / `result:*` 调用不会建立输入证据，不要用 `unsafe-coerce` 批量压制。

其他无法静态 specialization 的项目 method 也会在 strict mode 使用相同的 prefix/postfix error。
诊断会区分缺失 schema、Dynamic value/callable、legacy Optional、未绑定 generic/type-slot，以及显式
`:js-ffi` Dynamic boundary。按分类补 concrete/nominal schema、trait `:where` 或 entry slot binding。
`:js-ffi` 只声明 host capability，不授权运行时 method lookup；adapter 必须在返回业务层前转换 host
值或附加 external-object trait。

strict 项目源码也不能直接依赖 `&get-raw`、`record-get` / `&struct:get`、手写 `&%{}`，或缺少匹配 nominal
layout/index/tag evidence 的 `&struct:nth`。这些形式会报告 `E_RAW_PRIMITIVE_IN_TYPED_CODE`。
collection lookup 改用返回 `Option<T>` 的公开 API，Struct 字段改用 `(:field value)`，已知 Struct 构造改为直接调用
`StructName :field value`。
compiler/macro lowering、core internals、可复用 `defimpl`，以及 evidence 完整的 persisted indexed IR 保持可用。
已持久化的 `&%{}` IR 也可以保留，但必须能解析到具体 `defstruct`，并且每个声明字段恰好出现一次；
缺字段、重复字段或未知字段仍会被拒绝。

默认严格诊断下，`unsafe-coerce` 还必须位于当前 definition 的结构化 `Fn` schema 所声明的
`:features $ #{} :js-ffi` 词法范围内，否则报告 `E_UNSCOPED_UNSAFE_COERCE`。`js-ffi.raw.*`
一类 namespace 约定只用于 inventory，不授予权限。迁移时把 assertion 收拢到小 adapter，完成 host
value 的 validate/convert 后只返回 typed Calcit data；即使已正确标记，release quality gate 仍需审核
对应的 per-definition `unsafeCoerce` baseline。

#### `.trim` / `.blank?` 接收者迁移

`.trim` 与 `.blank?` 只对静态推断为 String 的接收者可用。若错误写成 `unknown method .trim for map`，
重点不是给 Map 增加方法，而是先修正数据流：当前接收者已被推断成 Map。可将接收者收紧/转换为 String，
或改成 `(trim receiver)` / `(blank? receiver)` 获取直接的参数类型诊断。新的 unknown-method 错误会同时写出
实际接收者类型和这两个函数形式；不要用 `unsafe-coerce` 掩盖业务数据类型错误。

#### 接收者方法与内部函数的边界

业务源码统一写接收者方法，例如 `xs .map f`、`m .keys` 和 `cell .deref`。当接收者类型已知时，
预处理器会根据类型及其 impl 表把调用专门化为对应的小写内部函数，例如 `&list:map`、`&map:keys`
或 `&atom:deref`；这些 `&scope:name` 名称是编译器、core 实现和生成代码之间的内部 ABI，诊断和
迁移建议不应要求业务代码手写完整内部名称。本阶段已让内建 Map/Ref 接收者生成直接内部调用；
自定义 nominal 类型会先按其 impl 表静态选中实现，而匿名函数实现到直接符号调用的代码生成仍会分阶段收紧。
默认严格模式拒绝未能静态专门化的 Dynamic 接收者方法调用，并报告 `E_DYNAMIC_METHOD_DISPATCH` 或
`E_DYNAMIC_POSTFIX_METHOD`；应通过补 schema、类型收窄或显式边界继续迁移。只有兼容模式可能保留历史动态分派。

这次收紧后，常见迁移如下：

| 旧的宽松写法 | 类型化写法 | 原因 |
| --- | --- | --- |
| `deref custom-box` | `custom-box .deref` | 公共 `deref` 只接受 `Ref<T>`；nominal 类型由自己的 impl 解析 |
| `keys struct-value` | `struct-value .to-map .keys` | `.keys` 的接收者是 `Map<K,V>`，Struct 字段来自静态定义 |
| `merge struct-value patch` | `struct-with struct-value (:field value)` | `merge` 保持同一 `Map<K,V>` 契约，不再兼作 Struct 更新 |
| `merge map-value nil` | `map-value` 或显式条件分支 | `nil` 不再充当空 Map；缺失状态应建模为 `Option<Map<K,V>>` |

`&map:keys` 在 native、JS 与 WASM 后端都返回 `Set<K>`，与公开 `.keys` schema 一致。若项目曾直接
依赖这个内部函数返回 List，应迁移为 `.keys` 并在确实需要顺序容器的位置显式转换。

`&list:nth` 是 core/backend 使用的内部、成功返回 `T` 的索引原语；越界或非整数索引现在会报错或 trap，
不会再以 `nil` 冒充 `T`。业务代码需要表达缺失时使用公开 `nth`，其结果是 `Option<T>`。

### 3.3 统一 entries

顶层 `:configs` 已停止被普通 runtime loader 加载。使用当前 Calcit 的一次性 formatter 迁移入口执行：

```bash
calcit calcit.cirru edit format
calcit calcit.cirru config show
```

`edit format` 会把旧 `:configs` 迁移为 `:entries.default` 并输出迁移计数；普通 runtime loader 不保留双格式解码分支。
若旧 `:configs` 与现有 `:entries.default` 冲突，或包含未知字段，formatter 会报错而不会静默覆盖。迁移后应审阅：

当前 formatter 还会把 `:files` 下的 namespace key 与 `:defs` 下的 definition key 统一写成 Symbol。loader 继续兼容旧 String key；若同一名字同时以 String 和 Symbol 出现，归一化后会明确报重名，避免静默覆盖。该变化只规范化标识符表示，不改变定义语义。

- 每个 entry 都有明确的 `:mode :native` 或 `:mode :js`。
- named entry 是完整配置，不继承 default 的 modules/type slots。
- `:init-fn` / `:reload-fn` 使用 definition symbol，而不是继续新增字符串值。
- entry 用 `:description` 说明用途，便于维护者和 Agent 选择正确入口。

### 3.4 Type slots

类库用 `deftype-slot` 声明由应用提供的编译期类型时，每个使用该 slot 的 entry 都要单独绑定：

```bash
calcit calcit.cirru config type-slots
calcit calcit.cirru config set-type-slot :dispatch-op app.schema/DispatchOp
calcit calcit.cirru config set-type-slot --entry test :dispatch-op app.test-schema/TestDispatchOp
```

未绑定 slot 会回退到 `:dynamic`，可能让 callback 检查和 method specialization 失去静态证据。升级后应逐 entry 检查，而不是只验证 default。

### 3.5 旧 schema 与动态类型

`:any` 只保留为 `:dynamic` 的兼容拼写；新 schema 不应继续引入。`edit format` 会输出 `W_LEGACY_ANY` / `W_DYNAMIC_TYPE_DEBT`，但不会猜测并自动改写类型关系。执行：

```bash
calcit calcit.cirru analyze check-types --summary-only
calcit calcit.cirru analyze check-types --deps --summary-only --format json
calcit calcit.cirru analyze weak-types \
  --only schema-dynamic,unresolved-type-slot,code-dynamic,code-nil \
  --intent unresolved,declared-unit,declared-optional \
  --summary-only
calcit calcit.cirru analyze deprecated --summary-only
```

兼容函数的 `:deprecated` 标签与定义文档也可由 `query context` 查询：

```bash
calcit calcit.cirru query context calcit.core/optionally --format edn
calcit calcit.cirru fix --preset surface-latest-v2 --include-attached --format edn
calcit calcit.cirru fix --preset core-api-0.28-v1 --include-attached --format edn
```

函数形式的 `optionally`、`join`、`vals`、`turn-str`、`turn-string`、`remove-watch`
均由其定义文档提供首选名，不需要另一份迁移名称表。
普通函数成功预处理时，弃用报告用编译器解析目标排除局部同名参数并识别 namespace alias；
quoted data 不计为调用，推荐方法复用旧内部 helper 也不计为旧函数调用。
无法预处理的定义保留保守 source 报告，仍需修正原编译问题后重新检查。
这些新增标签会改变旧项目的 `deprecatedCalls` 预算，随非 patch 升级交付；入口本身仍保留，
不要仅为压低预算删除断言或放宽类型。fix 的附带扫描需显式选择 `--include-attached`，
先审阅 preview，再以新 revision 应用，随后重放 `:tests` 和业务入口。

`case-default` 已标记 `:deprecated`，首选原生 `match`。所有模式都是字面量时两者等价，迁移时把第二个参数（默认值）挪到末尾的 `_` 分支：

```cirru.no-check
; 旧写法
case-default kind style-default
  :primary style-primary
  :danger style-danger

; 新写法
match kind
  :primary style-primary
  :danger style-danger
  _ style-default
```

字面量模式的调用可以批量迁移：

```bash
calcit calcit.cirru fix --rule case-default-to-match-v1 --include-attached --format edn
calcit calcit.cirru fix --rule case-default-to-match-v1 --include-attached --format edn \
  --apply --expect-revision <revision>
```

规则只自动改写与宏展开完全一致的调用，其余给出 `requires-review`。模式是表达式或变量时 `match` 不适用，改用 `cond`。`case` 暂未标记弃用，字面量模式下同样展开为 `match`。入口本身仍保留，随后续非 patch 版本再评估删除。

旧的 macro `Fn` / whole-`Dynamic` schema 不再作为运行时兼容格式：Snapshot loader 会在解析阶段以 definition 的完整 path 拒绝它。
若旧 Snapshot 已经包含结构化 `CodeEntry` 和这类 schema，应使用最终兼容版本 Calcit 0.13.51 先将模块改成严格
`Macro` contract（显式声明 `:required` / `:optional` / `:rest`、`:expansion` 和 `:capabilities`），再使用新版本检查。
若来源仍是更早的 direct-quote Snapshot，则直接用当前 `calcit calcit.cirru edit format`：隔离 formatter 会从参数形状生成
`Syntax` / `Expr<Dynamic>` / 空 capabilities 的保守严格 contract，避免要求一个无法读取该 direct-quote 格式的中间版本。
两条路径都必须审阅并收窄生成的 contract，并使用 `--deps` 检查实际解析的模块版本，不能用依赖仓库尚未发布的 main 代替。

有命中时去掉 `--summary-only` 查看 definition、Snapshot path、detail、suggestion 和 deprecated
目标文档。`check-types` 会把缺失或部分 schema（包括没有元素类型的 List/Map/Ref）列出来；
`weak-types` 同时区分 unresolved dynamic、unbound type slot、明确 JS FFI 边界、Unit nil 和旧 Optional 兼容债务；
`deprecated` 按调用位置指出已废弃 API。输入/输出共享类型时用 `:generics`，只约束能力时用 trait
`:where`，collection/ref 保留类型参数，有限异构值使用 enum。真正的 JS FFI 边界应显式标记
`:features $ #{} :js-ffi`，并在进入 typed code 前 validate/convert。无返回值使用 `Unit`；业务缺失
使用 `Option`，需要错误信息时使用 `Result`，不要让旧 `Optional<T>` 或裸 `nil` 无限保留。

strict mode 还会以 `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` 拒绝既没有结构化根 schema、也没有嵌入式
结构化 `Fn` hint 的可达项目函数，以及绕过 Snapshot loader、没有结构化根 schema 就直接进入预处理的
programmatically supplied macro；宏体内嵌套的函数 hint 不属于 macro contract。迁移时先声明完整 `Fn`
或 phase-aware `Macro` 结构；旧 Snapshot 中已有的嵌入式 `Fn` hint 仍可作为函数契约证据。正常从
Snapshot 加载的 legacy runtime `Fn` / whole-`Dynamic` macro 会更早在 loader
阶段携定义路径失败。若参数、返回值或宏表达式确实开放，只在对应位置写 `Dynamic` /
`Expr<Dynamic>`，不要用根 `Dynamic` 擦除整个契约。

当 `Dynamic` 实参进入重复出现的泛型位置时，strict mode 会进一步报告
`E_ERASED_GENERIC_RELATION`。例如 `Fn<T>(T) -> T`、同类型比较或参数化容器转换都依赖调用点保留
`T` 的关系；把未知值直接传入会让返回值或其他参数无法再被静态关联。应先 narrow/validate 成具体
类型，再调用泛型 API。确实开放的操作应收拢到一个小型 adapter，并让 adapter 的结构化契约明确
不承诺该泛型关系；开放容器直接用通用入口：`Map<K,Dynamic>` 用 `merge`，`List<Dynamic>` 用 `concat`，
泛型参数绑定为 `Dynamic` 即可，不要再在业务代码里直接使用 `&merge` / `&list:concat`。旧的
`merge-dynamic` / `concat-dynamic` 已弃用，由 `core-function-alias-v1` 迁移。
非 strict 模式保持原有兼容行为，便于渐进迁移。

当开放 `Dynamic`（或同形容器中的 `Dynamic` 成员）进入包含 Struct / Enum 的封闭参数契约时，
strict mode 报告 `E_DYNAMIC_NOMINAL_ARGUMENT`，并指出参数位置和目标契约。文本边界使用
`parse-cirru-edn-as` / `try-parse-cirru-edn-as`，已求值的 host/FFI 数据使用 `decode-map-as` /
`try-decode-map-as`；更复杂的 host 值在小型 typed FFI adapter 内完成验证和收窄。该规则不扩大为
所有 `Dynamic` 到 primitive 的调用，非 strict 模式也继续支持渐进迁移。

需要定位未能静态分派的调用时，可对每个 entry 运行只读动态方法报告：

```bash
calcit calcit.cirru analyze dynamic-methods --summary-only --format json
calcit calcit.cirru --entry test analyze dynamic-methods --format json
```

该报告只统计无法静态专门化的方法调用，不混入普通类型或 JS FFI warning；默认不统计依赖，
维护模块时可加 `--deps` 查看完整可达范围。报告本身不再提供 `--max` 数量门槛；
CI 应运行默认严格 `--check-only`，由编译器诊断决定类型正确性。

### 3.6 存量项目的类型收紧策略

先让默认严格 `--check-only`、definition `:tests`、目标后端行为测试通过；
`check-types`、`weak-types`、`deprecated` 是定位报告，不按命中数充当类型正确性门禁。
确实动态的 JS FFI 边界应显式声明和收窄，不要通过忽略 warning 伪造通过。

已有非零 `analyze quality` baseline 的旧项目可暂时在 CI 比较并逐步清零；新项目不再创建 baseline，
清零后从 CI 移除该命令。旧格式、按 definition 的预算及 0.14.x 迁移命令见
[历史版本迁移记录](upgrade-history.md#014x-质量-baseline-迁移)。

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test
```

#### 公共 equality 改为同类型契约

公共 `=`、`not=` 和 `/=` 只接受同一静态类型的操作数：`=` 的第一个参数和所有 rest 参数共享类型变量
`T`，另外两个函数的左右参数也共享 `T`。底层运行时仍能比较不同类别的值，但应用代码不能再依赖这种
隐式跨类型行为；它通常来自未收窄的 FFI 值、把类型谓词写成 equality，或遗漏了 nominal enum 的
pattern match。

升级后出现 `W_FN_ARG_TYPE_MISMATCH` 时，根据值的来源选择明确迁移：

- 两边本应是同一种数据：先 parse/normalize，再比较；
- 一边来自 Dynamic 或 FFI：在 adapter 中使用 typed FFI、validator 或 `assert-type` 收窄；
- 只是判断类别：使用 `string?`、`number?`、`tag?` 等类型谓词；
- 比较 Option/Result 或其他 nominal enum：先使用对应 predicate/method 或 pattern match，再比较 payload。

不要把业务调用改成底层 `&=` 来绕过检查。`&=` 和 `&compare` 保留跨类型运行时语义，只供经过审计的
core/runtime 边界使用，后续会作为独立的 internal runtime-polymorphic contract 继续跟踪。

### 3.7 format 的边界

`calcit edit format` 负责可解析性、canonical serialization 和已知旧结构迁移；它不是完整的语义 linter。告警写到 stderr 且不会阻止格式化。CI 需要在 format 后检查 `git diff`，并单独读取 `check-types --format json` 与 `weak-types --format edn`；前者目前仅支持 JSON 结构化输出，后者在 Calcit 工作流中优先使用 Cirru EDN。存量项目自己的质量阈值不得代替默认严格类型诊断。

### 3.8 Trait impl 从 tag method bag 迁移为 nominal impl

旧代码可能把 tag 作为 `defimpl` 的 trait 参数：

```cirru.no-check
defimpl :RenderImpl :Render $ .render
  fn (x) str x
```

这种 originless inherent method bag 已退役：`defimpl` 现在要求 impl 名与 trait 都是 symbol，遇到 tag 会以
`E_LEGACY_DEFIMPL_TAG` 失败。它本来也不会满足 `assert-traits`、函数/数据结构的 `:where` 约束，也不能被
`&trait-call` 选中。需要能力约束的代码应改成：

```cirru
let
    Render $ deftrait Render (.render :fn)
    RenderImpl $ defimpl RenderImpl Render
      .render $ fn (x) str x
  , RenderImpl
```

升级时还要修复以前被宽松实现接受的问题：

- concrete trait impl 必须完整实现声明的方法，且不能夹带 trait 未声明的方法；
- trait method 的值必须可调用；native 预处理在签名信息可用时还会检查函数签名；
- 同名方法不会跨 impl 拼接成一个 trait，也不会让两个独立 trait 互相冒充；
- list/map/set/string/number/struct/enum 等内建能力现在由 native 与 JS 共用的 nominal core impl 表提供。

建议在升级验证中显式覆盖两类负向用例：只有 `TraitA/.method` 时，`assert-traits value TraitB` 和 `&trait-call TraitB :method value` 都必须失败。

WASM 现在通过 `calcit wasm` 与 `calcit wasi` 提供公开 preview 命令，但仍不承诺 trait runtime table。能在预处理阶段消除的 trait 元数据仍可参与编译；残留的 `&impl::new`、`impl-traits` 或 `&assert-traits` 会明确报出“不支持 runtime trait table”，而不是静默返回 `nil`。现有 JS 生态可以继续以 JS 为主；希望迁移到 WASM 的项目应先用 `--check-only` 获取明确的 unsupported 边界，再逐步收窄宿主能力。

---

## 4）Yarn Berry 升级检查

### 4.1 packageManager 固定

```json
{
  "packageManager": "yarn@4.12.0"
}
```

### 4.2 CI 基础模板（GitHub Actions）

```yaml
- uses: actions/setup-node@v6
  with:
    node-version: 24

- name: Enable Corepack
  run: |
    corepack enable
    corepack prepare yarn@4.12.0 --activate
    yarn --version

- uses: calcit-lang/setup-calcit@v1

- name: Install deps
  run: caps --ci && yarn install --immutable

- name: Verify Calcit runtime toolchain
  run: caps verify --toolchain

- name: Validate Calcit snapshot and types
  run: |
    calcit calcit.cirru edit format
    git diff --exit-code -- calcit.cirru
    # `calcit config show` lists every configured :entries item, including default.
    while IFS= read -r entry; do
      if [ "$entry" = "default" ]; then
        calcit calcit.cirru --check-only
      else
        calcit calcit.cirru --entry "$entry" --check-only
      fi
    done < <(calcit calcit.cirru config show | awk '/^Snapshot Entries:/{in_entries=1; next} in_entries && /^  [^ ]/{print $1}')
    # 仅在仍有非零 legacy baseline 时保留：
    calcit calcit.cirru analyze quality --baseline config/calcit-quality.cirru

- name: Run project tests
  run: |
    calcit calcit.cirru test --tag unit --require-match --summary-only --format json
    calcit calcit.cirru --entry test
```

> ⚠️ CI 中安装的 Calcit 项目版本来自 `deps.cirru`，Action release 只决定安装器协议是否足够新。普通 workflow 不传 `version`；升级时修改 `deps.cirru`，并在需要新安装器能力时更新 `calcit-lang/setup-calcit`。不要在 `caps --ci` 之前运行 `calcit` 命令，否则会使用默认旧版模块缓存。完整模板见 [GitHub Actions](../installation/github-actions.md)。

说明：若项目依赖 `packageManager: "yarn@4.12.0"`，优先先执行 Corepack 激活，再让 CI 触发 Yarn。不要让 `setup-node` 的 Yarn cache 或其他 Yarn 调用早于 `corepack enable` / `corepack prepare`，否则可能误用 runner 上的全局 Yarn 1。 `caps --ci` 参数保证在 CI 加载模块时使用 HTTPS 协议，避免 CI 环境下的 SSH key 问题。

注意：`check-types`、`weak-types`、`deprecated` 仍是展示报告，不按命中数量失败。只有仍有非零
legacy baseline 的项目才在 CI 保留 `analyze quality --baseline ...`；baseline 清零后删除该命令，
不再把数量策略当作独立类型正确性判定。`--write-baseline` 仅允许在已存在的文件上降低预算，不能作为新项目初始化步骤。
`test --require-match` 会避免 tag 或 scope 写错后零测试仍退出成功。项目没有 named `test` entry 或
definition-attached unit tests 时，应删除对应示例行并替换成项目真实测试命令，而不是机械照抄。

### 4.3 lockfile 迁移

如果 `yarn install --immutable` 因 lockfile 格式变化失败：

1. 先执行一次 `yarn install` 生成新格式 lockfile；
2. 再执行 `yarn install --immutable` 做严格校验。

---

## 5）升级后最小验证矩阵

验收顺序与证据以页首 [当前升级闭环](#当前升级闭环) 第 4、8–11 步为准。下列检查补充该顺序中按项目情况才需要的部分：

1. `caps tree`：确认根开发依赖存在，传递模块的开发依赖未进入图
2. 需要定位未能静态分派的方法时，对相关 entry 运行只读 `analyze dynamic-methods --format json`
3. 非零 legacy baseline 项目继续运行 `analyze quality --baseline ...`；清零后删除该项（`check-types`、dynamic/nil `weak-types`、`deprecated` 仍只作为定位报告）
4. 声明支持 watch 的 entry 另行验收 `-w`；普通运行默认 once
5. `package.json` 中与编译/构建相关的脚本

### 老项目失败时的定位顺序

| 阶段 | 命令 | 主要发现 | 是否自动阻断 |
| --- | --- | --- | --- |
| 依赖图 | `caps tree/status/verify` | 递归版本、链接、store/native 收据 | 状态异常会阻断；普通图告警用 `--strict` 收紧 |
| Snapshot 规范化 | `calcit edit format` + `git diff` | 旧 configs/schema 拼写和规范化建议 | format 告警不阻断，diff 需人工审阅 |
| 问题写法 | `calcit fix --preset surface-latest-v2 --format edn` | 多表达式 body 与单表达式位置的冗余 `do`、旧数据 API、具名 `%::` / `%{}` 构造与可应用 replacement | preview 不写入；Cirru EDN 展开 rule IDs；apply 前后均需 staged validation |
| entry 预处理 | `calcit --entry ... --check-only` | 配置、缺失定义、参数/返回值、数据与 trait 类型错误 | 错误或 warning 均阻断 |
| 动态分派 | `calcit analyze dynamic-methods --format json` | 动态 receiver 与无法专门化的方法；默认排除依赖和无关 FFI warning | 只读定位；`--deps` 可审计依赖，正确性由 entry 预处理判断 |
| 静态债务 | `analyze weak-types --format edn`；`check-types/deprecated --format json` | 覆盖率、dynamic、nil/Optional、废弃调用 | 仅后两者暂未提供 EDN；报告本身不按命中数阻断，非零 legacy baseline 项目才继续比较 |
| 示例与测试 | `check-examples`、`docs check-md`、`calcit test --require-match` | API 示例、文档片段、definition-attached tests | 失败或未匹配测试时阻断 |
| 行为与后端 | entry、Node/Vite、项目测试 | native/JS/FFI 的真实行为差异 | 由进程退出码阻断 |

每次失败先查看终端诊断；需要完整运行栈时再看 `.calcit/error.cirru` 或执行
`calcit calcit.cirru query error`。针对单个定义可用 `calcit query context <ns/def> --format edn`，针对
具体表达式可用诊断返回的 Snapshot path 调用 `calcit query type-at <ns/def> --path code@... --format edn`；只有对接 JSON-only consumer 时才显式改用 JSON。

call graph 的 `--show-unused` 只能作为 entry-relative 线索；公开 API 和替代入口可能被列为 unreachable，不能据此自动删除。

## 限制

- Number 精确文本支持不允许把未经解码、无法证明 `ToString` 的开放 `Dynamic` 自动当作可转换值。
- 二义的 trait 调用仍可能被静态拒绝。
- 未证明的一等函数边界仍可能被静态拒绝。
