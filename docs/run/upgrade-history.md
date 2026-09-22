---
title: "Calcit 历史版本迁移记录（0.13–0.15）"
summary: "旧版严格诊断、修复规则、WASM/Component 边界和质量 baseline 的迁移记录"
scope: "core"
kind: "guide"
category: "run"
id: core/run/upgrade-history
related:
  - core/run/upgrade
  - core/run/library-quality
---

# Calcit 历史版本迁移记录（0.13–0.15）

这里保存历史版本当时的迁移步骤和能力边界，供旧项目定位；它不是当前版本的能力清单。
新项目和已升级项目请以[当前升级手册](upgrade.md)、
[WASM Component 边界](../installation/wasm-component-boundary.md)与当前 CLI 帮助为准。

## 0.14.15 的两阶段源码修复桥接

从较早的 0.14 项目升级时，先固定 Calcit 0.14.15，运行当时发布的
`tag-match-to-match-v1` 与 `required-struct-field-v1`，验证并提交后，
再切至当前工具链运行 `surface-latest-v2`。冻结的 `surface-latest-v1` 保持原有四条规则；
当前版本不恢复已退休 planner，preset 不能代替第一阶段。

## 0.14.20 工具链安装记录

当时文档使用已发布的 `calcit 0.14.20` 与独立的 `calcit-caps 0.1.1` 组合；
0.14.20 发布前曾要求继续固定 0.14.19。这是历史记录，不是当前推荐版本。
仍维护该版本的项目需分别核对 Calcit 与 caps 的 release 和项目 `deps.cirru`，
不要在新项目直接复制旧安装命令。

## 0.14.20 WASM 命令入口收敛

0.14.20 不再通过 `cargo install calcit` 或 GitHub Release 发布 `cr-wasm`。项目与 Agent 应直接选择宿主契约：

- 原来的 `cr-wasm <snapshot> --target core` 改为 `calcit wasm <snapshot>`；
- 原来的 `cr-wasm <snapshot> --target wasi` 改为 `calcit wasi <snapshot>`；
- 两者的 `--check-only`、`--emit-path`、`--init-fn`、`--reload-fn` 与 `--entry` 继续由公开子命令提供。

这不是能力删除：Snapshot 加载、预处理、target validation 与 codegen 仍共用同一实现。仓库为 WASI 自举保留
feature-gated 的内部回归 harness，但它不是兼容 CLI，也不应被用户脚本调用。遇到旧 workflow 时应显式改写命令，
不要尝试复制或重新发布该内部 binary。

## 0.14.20 Component preview

0.14.20 新增 opt-in 的同步 Component core boundary，不改变普通 native、JavaScript、
`calcit wasm` 或 `calcit wasi` 项目的默认行为。只有显式使用
`calcit ffi export --boundary component` 或 `calcit wasm --boundary component` 的项目
需要关注：当时支持 `Unit` 结果、`Bool`、`Buffer`、`Number`、UTF-8 `String`、递归同质
`List<T>`，以及闭合单态 `Option<T>` / `Result<T,E>` 的 import/export adapter，默认 contract
格式为 Cirru EDN。独立的 `calcit-bindgen` 可为这些类型生成 WIT 并包装 runnable Component；
Struct record、普通 Enum variant、明确宽度数值与 async 在该 preview 中尚未完成，不会静默退化为 Dynamic
或 core 私有 value ABI。

## 0.15.1 Struct/Enum Component boundary

0.15.1 完成 monomorphic Struct record 与闭合单态 Enum variant 的同步 Component boundary。Struct 字段名称、顺序和类型完全来自
规范化的 `defstruct` schema；嵌套 Struct 与已经支持的闭合字段类型复用统一递归 walker。边界不会根据运行时值
补字段或猜测 layout，未具体化 generic、Dynamic 字段和其他未支持类型会带 schema path 在生成阶段失败。

Enum 的 case 名称与顺序来自规范化后的 `defenum` schema，并决定从零开始的 Canonical ABI discriminant。无 payload、
单 payload 与多 payload case 分别映射为 WIT bare case、直接 payload 与 tuple payload；payload 可递归包含已支持的
闭合类型和 Struct。anonymous/open Enum、递归 Enum、未具体化 generic 与显式 `Unit` payload 会明确拒绝，不会退化为 Dynamic。

该阶段没有新增命令：core 继续使用 `calcit ffi export --boundary component` 与
`calcit wasm --boundary component`，WIT 与 runnable Component packaging 继续由 calcit-bindgen 的
`generate` / `check` 负责。Struct/Enum/Result 已在 Wasmtime 与 jco/Node 完成实际往返；明确宽度数值的
Component lowering 见下一节。

从 0.14.20 preview 升级时不需要改写命令，也不要新增 WIT/component wrapper。应重新执行
`calcit ffi export --boundary component` 导出 contract，用 calcit-bindgen `check` 查看 ABI fingerprint 与兼容性变化，
确认后再用 `generate` 刷新 WIT、manifest 和 Component 产物。依赖 declaration 顺序的 Struct field 与 Enum case 会进入
公开 ABI；若顺序发生变化，应把它作为显式接口变更审阅，并重新运行宿主侧 Wasmtime 或 jco 往返测试。

## 0.15.2 明确宽度数值边界

0.15.2 让十种明确宽度数值 refinement 直接进入 Component Canonical ABI，同时保持单一运行时数值表示。

运行时值形状不变：signed/unsigned 8/16/32/64 位整数与 32/64 位浮点数是 `Number` 的静态边界 refinement，
native 仍是 `f64`，JavaScript 仍是 `Number`。refinement 可安全地作为 `Number` 使用，普通 `Number` 不会隐式收窄；
普通算术返回 `Number`，不会保留已经失效的范围证明。

进入明确边界必须显式转换：`number->int8`、`number->uint8`、`number->int16`、`number->uint16`、`number->int32`、
`number->uint32`、`number->int64`、`number->uint64`、`number->float32`、`number->float64` 返回
`Result<目标类型,String>`，拒绝非整数、符号错误、溢出、NaN/Infinity 与不可接受的精度损失——不截断、不环绕、
不静默舍入。Calcit 的 `f64` 表示无法无损覆盖全部 64 位整数，因此只承诺可精确表示的安全整数范围，host 超范围
值明确拒绝。

Component boundary 按推导并规范化后的 schema 宽度确定性导出 `Int8`、`UInt8`、`Int16`、`UInt16`、`Int32`、
`UInt32`、`Int64`、`UInt64`、`Float32`、`Float64` 独立 kind；普通 `Number` 仍导出为 `number`，bindgen 不根据值、
名称或调用位置猜测宽度。从 0.15.1 升级时不需要新增顶层命令、语法或 analyzer；但必须重新执行
`calcit ffi export --boundary component`，并用 calcit-bindgen `generate` / `check` 刷新 WIT、manifest 与 Component
产物，不保留旧数值 ABI 兼容层。旧边界写法只有在能证明等价时才由 `calcit fix` 给出迁移建议，其他仍需人工决定。

## 0.15.3 异步 Component export 基础

0.15.3 开始消费函数 schema 已有的 `:async true`，不增加新的表层 Task/Future 类型或 CLI。Component Interface IR v4
将该标记导出为 `invocation: async`；`calcit wasm --boundary component` 为对应 export 生成 WASI 0.3 async core shape：
参数继续使用现有 Canonical ABI 类型 walker，core 函数没有返回值，逻辑返回值通过 packaging 注入的强类型
`task.return` 恰好完成一次。

这是分阶段开放的能力。async import 对最多 4 个 flat 参数使用 direct shape；更多参数会写入按 Canonical ABI
布局的 parameter record，并传递单个 pointer。两种形状都处理立即完成以及 subtask 的
`starting` / `started` / `returned`、终态清理和 drop。
不要把 native `async-task-v1` 队列、句柄或 polling API 搬到 Component 接口，也不要把 async import 临时改成同步 ABI。
升级相关项目时，先重新导出当前 contract（native 为 IR v3，Component 为 IR v4），再升级 calcit-bindgen：除连接
`[export]$root/[task-return]<export-symbol>` 外，还需连接 `$root` 下的 `[waitable-set-*]`、`[waitable-join]`
与 `[subtask-drop]` canonical builtins。async import/export 分别使用 `[async-lower]` 与
`[async-lift-stackful]` 名称前缀，使 `wit-component` 能按 Canonical ABI 识别这些内部接缝。当时的 stackful adapter
对已收到的 cancellation 终态清理后 trap；主动取消留给 callback cancellation 阶段。

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

适用对象：通过 Calcit CLI 运行并产出 JS 的项目（例如 Respo）。

升级完成的标准不是“`caps upgrade --all` 执行成功”，而是：

- 新版 `calcit` 与独立版本的 `caps` 已分别固定到本地和 CI，且 `deps.cirru`、`calcit`、`@calcit/procs` 版本链路一致；
- 每个声明支持的 entry 都通过 `--check-only`，并完成对应 native / JS 行为测试；
- `check-types`、`weak-types`、`deprecated` 已生成可复查报告，存量债务有 baseline，新增债务被阻断；
- examples、Markdown 示例、项目测试与真实消费者回归覆盖了公开能力。

---

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
