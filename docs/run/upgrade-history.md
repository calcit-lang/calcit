---
title: "Calcit 历史版本迁移记录（0.13–0.19）"
summary: "0.14 两阶段修复桥接、默认严格诊断、质量 baseline 与 0.19 兼容清理的迁移记录"
scope: "core"
kind: "guide"
category: "run"
id: core/run/upgrade-history
related:
  - core/run/upgrade
  - core/run/library-quality
---

# Calcit 历史版本迁移记录（0.13–0.19）

这里保存历史版本当时的迁移步骤和能力边界，供旧项目定位；它不是当前版本的能力清单。
各固定桥梁的适用写法、所用 CLI 与退场状态汇总在[当前升级手册的历史迁移桥梁](upgrade.md#历史迁移桥梁)。
新项目和已升级项目请以[当前升级手册](upgrade.md)、
[WASM Component 边界](../installation/wasm-component-boundary.md)与当前 CLI 帮助为准。

## 0.14.15 的两阶段源码修复桥接

从较早的 0.14 项目升级时，先固定 Calcit 0.14.15，运行当时发布的
`tag-match-to-match-v1` 与 `required-struct-field-v1`，验证并提交后，
再切至当前工具链运行 `surface-latest-v2`。冻结的 `surface-latest-v1` 保持原有四条规则；
当前版本不恢复已退休 planner，preset 不能代替第一阶段。

## 0.14 默认严格诊断

Calcit 0.14 起，普通运行、`--check-only` 和代码生成默认启用严格预处理诊断；无需再通过
`--strict-types` 才把不安全类型路径提升为稳定的 `E_*` 错误。`--strict-types` 仍可显式确认严格策略并在运行或代码生成前预检查入口，不再运行数量预算。尚未迁移完成的旧项目可以暂时显式使用
`--compat-types` 恢复 0.14 之前的 warning 行为；两个开关互斥。

推荐先通过 0.13.79 这个迁移桥接版本清理 warning 和质量报告，再升级到 0.14：

```bash
calcit calcit.cirru --compat-types --check-only  # 临时保留旧行为
calcit calcit.cirru --check-only                 # 0.14 默认严格诊断
calcit calcit.cirru --strict-types --check-only  # 显式执行严格入口检查
```

`--compat-types` 只用于限时迁移，不应成为新项目或长期 CI 的默认参数。已经在 entry 中显式配置的
feature policy 仍然生效；兼容开关不会覆写 Snapshot。

## 0.14.x 质量 baseline 迁移

三类命令的退出语义不同，不能只看命令是否成功：

- `--check-only` 和实际 native/JS codegen：预处理错误或 warning 会阻断并非零退出，必须修到通过；
- `check-examples`、`docs check-md` 和 `calcit test`：所选示例/测试失败时阻断；测试应加
  `--require-match`，避免过滤条件拼错后“零测试通过”；
- `check-types`、`weak-types`、`deprecated`：是静态定位报告，有命中不等于非零退出；已有 `analyze quality`
  baseline 在 0.14.x 只作为存量项目清债 ratchet，不是类型正确性的另一套判定。

老项目不必在第一次升级提交中把所有历史 Dynamic 清零，但必须先让严格检查通过，再逐步缩小边界：

1. 已有 baseline 的项目继续记录 Calcit 版本与 Snapshot revision；没有 baseline 的项目不再新建；
2. 先要求 `--check-only`、测试和行为构建全绿；
3. 现有 baseline 只允许按模块降低，不能无说明地更新；
4. baseline 清零后从 CI 删除对应命令和文件，继续依赖严格检查、测试和目标后端验证；
5. 对确实动态的 JS FFI 边界显式声明 `:features $ #{} :js-ffi`，不要用 ignore warning 伪造通过。

baseline 不要只保存一个总数。类型覆盖至少比较 `levels.none` 和
`levels.none + levels.partial`（未完全覆盖总数）：`none` 变成 `partial` 是进步，不应因为
`partial` 单项上升而失败。弱类型则分别比较 `kinds.schema-dynamic` / `unresolved-type-slot` / `code-dynamic` / `code-nil`
和 `intents.declared-optional`，再比较 `deprecated` 的 `summary.calls`。原生 v2 或重新生成的 baseline
还应比较 `unsafe-coerce` 的 occurrence 数；旧原生 v1 和扁平 baseline 只约束原有八项指标，不要求提供该数据。否则一种债务增加、另一种
减少时，相同的总数会掩盖回归。

新项目直接执行默认严格检查，并运行实际目标测试：

```bash
calcit calcit.cirru --check-only
calcit calcit.cirru --entry test
```

已经提交 baseline 的存量项目可在 0.14.x 继续执行比较：

```bash
calcit calcit.cirru analyze quality --baseline config/calcit-quality.cirru
```

原生 v2 baseline 记录 scope、汇总指标和每个 definition 的独立预算，并将 `unsafeCoerce` 作为单独的 host-boundary 预算。新增 definition 默认预算为零；
一个 definition 的改善不能掩盖另一个 definition 的回归。`--write-baseline` 仅在现有文件上降低或保持预算，并原子写入；
不能用于新建 baseline 或提高任何 definition 的指标。baseline 仍需人工审阅并随仓库提交，清零后应删除。

例如把首次审阅后的上限提交为 `config/calcit-upgrade-baseline.cirru`：

```cirru
{} (:typeNone 4) (:typeNotFull 22) (:schemaDynamic 21)
  :codeDynamic 0
  :codeNil 22
  :unresolved 43
  :declaredOptional 0
  :deprecatedCalls 0
```

这个旧版扁平 shape 仍可直接传给 `analyze quality --baseline`，便于已有项目删除 Node 检查脚本后
无缝迁移，并继续执行原本八项指标；在现有文件上执行 `--write-baseline` 会生成 v2 的按 definition 格式，并要求新增的 `unsafeCoerce` 指标为零或不超过旧预算。如果迁移把 `none` 改善为
`partial`，`typeNone` 会下降且 `typeNotFull` 不变；改善为 `full` 时二者都会下降。确有类型债务在
不同分类间迁移时，不应通过自动写入抬高另一项预算；先依默认诊断与测试解决问题，确需调整存量文件时单独人工审阅。baseline 归零后
删除 `analyze quality` 调用；0.15 将不再把 coverage/Dynamic 数量作为独立类型正确性策略。

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
[历史两阶段桥接步骤](#01415-的两阶段源码修复桥接)完成旧版迁移，
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
