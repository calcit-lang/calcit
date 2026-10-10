---
title: "Compiler-guided Source Fixes"
summary: "使用 calcit fix 预览编译器指导型迁移，或审阅并原子应用项目级结构改写"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "calcit fix"
  - "source migration"
  - "automatic migration"
entry_for:
  - "calcit fix"
id: core/run/fix
parent: core/run
related:
  - core/run/edit-tree
  - core/run/upgrade
  - core/run/workflow-entrypoints
requires:
  - core/agent
leads_to:
  - core/run/upgrade
---

# Compiler-guided Source Fixes

`calcit fix` 是所有“检测并改写可证明问题”的统一入口，不再为单条规则增加 `analyze detect-*` 或新的顶层命令。
入口分工与兼容读取边界见 [Calcit CLI 工作流入口收敛](workflow-entrypoints.md)。

项目自己的旧 helper 或调用形状可使用本页的 `--pattern` / `--replace`，不需要给编译器增加专用迁移规则。
该模式是需人工审阅的语法改写，不属于下文可证明等价的自动迁移。

## 项目级结构改写

使用一个 Cirru 表达式作为模式和模板，`?name` 绑定完整子树。表达式最外层不需要括号：

```bash
calcit calcit.cirru fix --pattern 'old-helper ?xs ?sep' \
  --replace '.join-string ?xs ?sep' --include-attached --format edn
calcit calcit.cirru fix --pattern 'old-helper ?xs ?sep' \
  --replace '.join-string ?xs ?sep' --include-attached \
  --apply --expect-revision '<预览中的 revision>' --format edn
calcit --check-only
calcit test '<修改后的 namespace/definition>' --require-match
```

预览仅扫描项目源码，逐处给出位置、原始/替换 AST 与 fingerprint，不写入或执行这些表达式。
默认跨项目 namespace；`--ns` / `--def` 可限定范围。`--include-attached` 同时处理 `:tests` 和 `:examples`，
不传时报告这些区域仍需手动审阅。quote/quasiquote（含限定名）按语法保守跳过；祖先模板也不能改变、丢弃或复制其中的数据，完整保留 quoted 参数的替换仍可预览。不改 namespace imports、schema、doc 或 tags。
同名变量要求捕获的子树相等；模板中的变量必须在模式中出现。变量在模式与模板中的出现次数必须相同，
否则拒绝静默丢弃或复制表达式。嵌套的原始匹配在同一次事务中合成，不再次匹配新生成的代码。

所有候选始终标为 `requires-review`：操作者须核对 binding、求值顺序、失败行为与业务含义。
变量次数相同、类型检查成功都不证明语义等价；`--apply` 表示明确采用已审阅的模式和模板，不把候选升级为 `machine-applicable`。
应用沿用 revision、VCS、工具链版本与原子事务门禁，先在暂存 Snapshot 严格预处理所选源码；纳入附带区域时也检查测试与示例，
不执行它们的运行时主体。任何暂存错误或 warning 都拒绝发布整批修改，并报告暂存诊断，原文件不变。
旧源码本身可以不通过类型检查，只要明确审阅后的候选通过门禁；这不会自动增加 schema、cast、Dynamic 或 FFI 权限。
成功后仍需运行原断言以及项目目标的 native/JS/WASM 检查。此模式不能与 `--rule`、`--preset`、`--workflow`、`--verify` 或 `--to` 混用。

## 编译器指导型迁移

`calcit fix` 把编译器已经能够唯一判断的迁移建议映射回 Snapshot source AST。默认命令只做预览；它会在同目录的
staged Snapshot 上尝试替换并重新编译选定 scope，成功后仍不写入原文件：

```bash
calcit calcit.cirru fix
calcit calcit.cirru fix --ns app.main --def render! --format edn
```

升级函数形式兼容名时，可显式把附带测试与示例纳入同一次预览：

```bash
calcit calcit.cirru fix --rule core-function-alias-v1 --include-attached --format edn
calcit calcit.cirru fix --rule core-collection-len-v1 --include-attached --format edn
calcit calcit.cirru fix --preset core-api-0.28-v1 --include-attached --format edn
calcit calcit.cirru fix --preset surface-latest-v2 --include-attached --format edn
```

该选择复用编译器的引用解析与 Snapshot 原子事务。一个测试或示例区域里的多个旧名合并成一个操作，
保留断言、标签、实参顺序与示例顺序。函数旧名的引号、一等函数身份和未知宏边界仍返回 `requires-review`；
方法规则保留 quoted data，无法证明接收者或宏来源时要求 review，局部同名 binding 不改写。
预览不写入文件；应用时仍须核对 revision 并传 `--expect-revision`，随后运行附带测试。
不传 `--include-attached` 时仍只扫描 `:code`。

当前支持显式的 `core-function-alias-v1`、integer predicate 规则，以及 List add、
collection len 和 effect method 等已有方法等价改名规则。
附带方法改写与 `:code` 共享接收者类型、方法实现和源码来源的证明，局部 `let` 类型由实际预处理结果保留。
`round?` 的 reader 调用头与 Number 方法迁移同时覆盖，仍保留引号、局部遮蔽和未知宏保护。
一个示例区域中混有需要 review 的表达式时，整个区域暂不自动改写。
`redundant-do-v1` 与 `single-expression-do-v1` 也支持附带区域，复用原 body 规则。
最外层多表达式 `do` 保留，只有真实 variadic body 内的 wrapper 能拆开；单表达式 wrapper 可直接解包。
quoted data 不改写，未知宏中无法证明执行上下文的区域保留 review。
`core-nominal-constructor-v1` 支持把附带调用的 `%some` / `%none` / `%ok` / `%err` 改为 Option/Result 直接构造。
嵌套 payload 保留原位置并只求值一次；一等函数值、名义类型遮蔽或未知宏来源要求 review。
项目的 `named-enum-constructor-v1`、`named-struct-constructor-v1` 也支持附带区域，
复用定义来源、字段完整性和嵌套 payload 证明，保留字段在源码中的求值顺序。
`core-option-method-v1`、`core-result-method-v1` 支持附带 helper 调用：
接收者和 fallback 的类型证明、方法实现与宏来源要求和普通定义相同。
局部绑定只收集真实绑定位置，初始化表达式中的类型名不视为遮蔽。
转换规则 `core-identity-conversion-v1`、`core-list-add-v1` 和 `core-collection-len-v1` 也支持附带区域，
与普通定义共用实参类型、方法实现和源码上下文证明；局部接收者保留实际预处理类型，
reader 展开的调用不会被误认成其中叶节点的类型。Set `.add` 不被 List 规则改写，
未知宏保留 review，quoted data 原样保留。
`core-api-0.28-v1 --include-attached` 把该 preset 的 5 条规则组合应用于附带区域（0.29.0 起去掉了随旧名退役的
10 条别名规则，原 15 条以 0.28.x 的 CLI 为准），
不改变不传 flag 时的扫描范围。同一区域中有需要 review 的表达式时不部分写入。
`removed-data-api-v1` 与 `surface-latest-v1/v2` 也支持附带区域；旧 API 使用编译器的现有诊断定位，
`tuple?` 的值/定义含义选择仍需人工 review。组合规则逐步重新证明 source，整个测试或示例区域仍只产生一次原子替换。
其他未覆盖规则的附带组合会列出不支持的规则并报错，不会静默跳过。
下文可自动迁移的等价规则默认扫描 `:code`，附带扫描统一由本节的 `--include-attached` 控制。

不带 `--ns` 的整项目预览覆盖所有 named entry 的源码，因此按 target 中立的方式规划改写；例如 browser 默认入口不会阻止扫描
server-only 的 Node FFI 定义。这不表示每个入口都能运行所有定义，也不能代替逐入口的 `--check-only` 或 `fix --workflow strict --verify`。
限定 `--ns`/`--def` 时仍使用所选 `--entry` 的 target 检查；检查 Node-only 定义时应显式选择 Node entry。

当前稳定规则包括：

- `removed-data-api-v1` 复用编译器已有的 `W_REMOVED_DATA_API` 解析结果。只有一对一的名称迁移
  （例如 `tuple-enum` 到 `enum-definition`）标为 `machine-applicable`；`tuple?` 需要在值判断与定义判断之间选择，因而只返回
  `requires-review`，其 `replacement` 为 `null`。
- `redundant-do-v1` 整理 `defn`、`fn`、`let`、`let[]` 及嵌套 `do` 的 variadic body。父结构本来就按顺序执行多项、
  并以最后一项作为返回值时，这条规则会把直接子节点的单层 `do` splice 到父 body。`if` 分支、调用参数、binding value
  以及 `defmacro`、`quote`/`quasiquote` 数据不在自动修改范围内；macro 是否把多项 body 打包成一个表达式需要保留显式语义。
- `single-expression-do-v1` 解包恰好只有一个 payload 的 `(do expr)`。这时 `do` 在分支、调用参数和 binding value
  中也没有组合多个步骤，不会补充求值、失败或类型语义；`defmacro`、`quote`/`quasiquote` 仍作为宏/数据边界保留。
- `named-enum-constructor-v1` 把能静态解析到项目 `defenum` 的 `%:: Result :ok value` 改为
  `Result :ok value`。`calcit.core` 的 enum（如 `Option`、`Result`、`MapEntryDecision`）在所在 namespace
  没有同名定义或 import、也没有同名局部绑定时同样改写，例如 `%:: Option :some x` 改为 `Option :some x`；
  编译器 trace 必须确认原型解析到 `calcit.core` 的同名定义。
- `named-struct-constructor-v1` 把能静态解析到项目 `defstruct` 的 `%{} Person (:name name)` 改为
  `Person :name name`，并保持字段表达式的原始求值顺序。
- 两条 named constructor 规则也改写 `quasiquote` 模板中（`~` / `~@` 之外）的构造：模板在每次展开时才编译，
  因此不依赖编译器 trace，而是按宏所在 namespace 静态解析原型并检查字段完整性。带 namespace 前缀的原型
  （如 `schema/Effect`）在展开处仍按宏所在 namespace 解析，属于 `machine-applicable`；无前缀原型在展开处
  可能被局部绑定捕获，给出 `requires-review`。原型本身是 `~proto` 或字段由 `~@` 拼接时不给建议；
  `defmacro` 中模板外的展开期代码与 `quote` 数据保持不变。
- `core-nominal-constructor-v1` 是 0.25.0 的 Option/Result 构造器迁移试点：把编译器已解析到
  `calcit.core/%some/%none/%ok/%err`、且实参数量匹配的调用改为 `Option :some/:none` 或
  `Result :ok/:err`。嵌套构造在一次 guarded subtree replacement 内完成，每个 payload 仍按原位置
  求值一次；局部遮蔽、函数值引用和未经证明的 macro 展开不自动改写。当前须显式指定 `--rule`，
  不静默修改已有升级 preset；旧 helper 从 0.25.0 标记弃用，在真实消费者迁移且至少经过一个版本窗口后
  才考虑移除，最早为 0.26.0。试点只扫描所选 definition 的源码 `:code`；definition-attached
  `:tests`、`:examples`、macro 定义与无法映射回源码的展开引用暂不自动改写，须人工检查。
- `core-option-method-v1` 在原有取值迁移之外，也把 `option:some?` / `option:none?` 改为
  `value .some?` / `value .none?`。只有编译器已解析到对应 core helper、接收者静态类型
  能证明分派到同一个 Option 方法的源码调用才自动改写。取值调用仍改为 `value .unwrap`
  或 `value .unwrap-or fallback`；唯一的空值 fallback 特例是
  `option:unwrap-or (%none) fallback` 与 `option:unwrap-or (Option :none) fallback`：源码引用须证明构造器
  来自 core、没有 payload，且 fallback 有具体类型、目标方法契约为 `proven`，才允许改写。
  源码中推断为短名
  `Option<T>` 时先按当前命名空间解析；同名项目类型不得仅凭拼写当成 core Option。接收者和备用值仍
  各求值一次、顺序不变；除此空值特例外，开放的 `Option<Dynamic>`、函数值引用、未知 macro 来源或无法回溯
  的类型证据只报告 `requires-review`。已核实会单次保留调用的 core `let`、`cond`、`do`、
  `fn`、`assert=` 等展开可通过宏边界，但仍必须满足同一静态类型与方法契约；其他宏不自动放行。
  当前须显式指定 `--rule`，不改写已发布的 preset，
  也不把内部 helper 立即删除。默认只覆盖定义的 `:code`；显式传 `--include-attached` 时，
  附带 `:tests` / `:examples` 使用同一证明。先预览、核对来源和 revision，再应用并运行严格检查与业务测试。
- `core-result-method-v1` 把 `result:ok?` / `result:err?` 改为 `value .ok?` / `value .err?`，
  也处理 `result:unwrap-or result fallback` 到 `result .unwrap-or fallback`；仍须证明 core 来源和
  同一个接收者方法契约，不因谓词返回 Bool 就跳过接收者类型检查。
  已证明来自 core 的 `%err error` 或 `Result :err error` 没有成功值，因而可由具体 fallback
  确定成功类型，同时保留错误类型；方法契约仍须为 `proven`，源码引用、求值次数和顺序也须可证明。
  成功值有具体类型、方法契约已证明的 core `%ok` 也可迁移；备用值不能代替成功值的类型证据。
  普通 `Result<Dynamic,E>`、成功值来自 Dynamic 的 `%ok` / `Result :ok`、同名类型遮蔽、
  函数值和未知 macro 只给 `requires-review`；具名构造若缺少精确源码类型证据同样保留审阅。
  与 Option 规则一样，本规则须显式选择，不修改已发布 preset；通过 `--include-attached` 纳入 `:tests` / `:examples`。
  先运行 `calcit calcit.cirru fix --rule core-result-method-v1 --format edn` 预览，再带原样
  `--expect-revision` 应用；重复预览应为空，随后运行严格类型检查和项目测试。
  0.26.0 不删除 Option/Result 旧方法 helper；它们仍是 core method 的实现目标。应用可直接调用的
  兼容入口最早于 0.27.0 且满足[退场条件](../features/api-roles.md#旧方法-helper-的退场条件)后才考虑移除，
  不能把一次空预览误认为已完成消费者迁移。
- `core-integer-predicate-v1` 把 Cirru reader 直接解析为内建 Proc 的单参数 `round?` 调用头改为
  `calcit.core/integer?`。新入口复用有限且恰好没有小数部分的既有语义；不会把 Bool 当作整数类型 refinement。仅改写可回溯的源码调用，保留实参原位与求值次数；含同名词法绑定的定义及 quoted 数据跳过，未知 macro 只给 `requires-review`。
  Number `.round?` 已在 0.29.0 删除（`E_RETIRED_METHOD`）；0.28.x 的同名规则可把静态 Number 接收者的 `.round?` 改成 `.integer?`，需在升级 CLI 前运行。一等函数引用不自动修改。使用 `calcit calcit.cirru fix --rule core-integer-predicate-v1 --format edn` 预览，核对来源与 revision 后应用并重复预览；包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-identity-conversion-v1` 把参数已证明为 String 的内建 `turn-tag` / `turn-symbol` 完整调用分别改为 `calcit.core/to-tag` / `calcit.core/to-symbol`；也把解析到 core 兼容函数、且参数已证明为内建 Nil、Bool、Number、String、Tag 或 Symbol 的 `turn-string` 改为 `calcit.core/to-string`。这些标量路径调用相同的底层转换，实参仍只求值一次；自定义 `ToString` 实现、Dynamic、已知不支持的集合、局部同名绑定、quoted 数据、未知 macro 与一等函数值都不自动改写。先用 `calcit calcit.cirru fix --rule core-identity-conversion-v1 --format edn` 预览，核对来源与 revision，再带 `--expect-revision` 应用并复查；包含在 `core-api-0.28-v1`；旧 surface preset 不变。WASM 仍不支持动态 Tag/Symbol intern，迁移不意味着获得 WASM 支持。
- 0.29.0 起，`core-set-include-v1`、`core-list-fold-v1`、`core-list-flat-map-v1`、`core-list-join-string-v1`、
  `core-list-get-v1`、`core-collection-combine-v1`、`core-predicate-method-v1` 随 List `.reduce` / `.bind` / `.join-str` / `.nth`、
  Map/Set `.mappend`、Set `.add`、Map/Set `.contains?` 与 Map `.includes?` 一起退役；Map `.add` 没有迁移规则，需人工改写为 `.assoc key value`。这些规则只在 0.28.x 的 CLI 中提供，需在升级 CLI
  前运行；当前 CLI 用 `--rule` 选择它们时会报错并给出该提示，残留调用由严格检查的 `E_RETIRED_METHOD` 定位。删除项与迁移命令见
  [升级指南](upgrade.md#兼容入口的退场节奏)。
- `core-non-nil-predicate-v1` 随函数 `some?` 在 0.29.0 退役，只在 0.28.x 的 CLI 中提供：它把编译器已解析到
  `calcit.core/some?` 的源码引用改为 `calcit.core/non-nil?`。当前 CLI 用 `--rule` 选择它时会报错并给出该提示；残留的
  `some?` 调用在检查时报告未知名字，告警末尾给出 `non-nil?`。同版本删除的函数 `join-str`、`add-watch`、`cpu-time`
  分别改写为 `join-string`、`add-watch!`、`monotonic-time-ms`，残留调用得到同样的提示。
- `core-function-alias-v1` 把编译器已解析到 core 兼容函数的源码引用改为首选名：`optionally` → `nil->option`、`join` → `intersperse`、`vals` → `distinct-values`、`section-by` → `chunks`、`merge-dynamic` → `merge`、`concat-dynamic` → `concat`（`join-str` 已在 0.29.0 删除，见上一条）。每对新名都由旧名转发全部参数，因此只改名字，参数求值次数与结果不变；局部同名 binding、quoted data 与未知 macro 不自动改写，跨 macro 的引用只给 `requires-review`；作为一等值（非调用头）使用的引用也只给 `requires-review`，因为新名是独立函数，函数身份不同。这是独立的显式规则，**不**包含在已发布的 `core-api-0.28-v1`；用 `calcit calcit.cirru fix --rule core-function-alias-v1 --format edn` 预览，核对 revision 后带 `--expect-revision` 应用并重复预览。注意 `join` 实际返回插入分隔符的 List 而不是字符串，改名不改变这一行为；需要字符串时应改用 `join-string`，该判断需人工完成。
- `core-ref-constructor-v1` 把 Ref 构造的兼容旧名改为首选名：`atom` → `ref`、`defatom` → `defref`。读取器把 `atom` 与 `ref` 解析为同一个内建实现，因此代码中所有被求值的 `atom` 都直接改写，Ref 身份与求值次数不变；`defatom` 是按名字解析的 core 语法，只在调用头位置、且所在 namespace 没有同名定义或 import、没有同名局部绑定、不在 quasiquote 模板中、外层也没有未知 macro 时自动改写，其余情况给出 `requires-review`。quoted data 与注释跳过。先用 `calcit calcit.cirru fix --rule core-ref-constructor-v1 --include-attached --format edn` 预览，核对 revision 后带 `--expect-revision` 应用，重复预览只应剩下需要 review 的项。本规则包含在 `core-api-0.29-v1`，`core-api-0.28-v1` 与 surface preset 保持原定义。
- `case-default-to-match-v1` 把已弃用的 `case-default item default (pattern value)...` 改写为 `match item (pattern value)... (_ default)`。只有全部模式都是字面量（tag、字符串、数字、bool）时才自动改写：此时 `case-default` 宏自身就展开为这段 `match`，求值次数、未命中时的默认值与 native/JS/WASM 行为都不变。嵌套的 `case-default` 合并在外层的一次替换中。模式含表达式、变量或 enum tuple（宏会走 `&case` 比较值）、调用位于 quasiquote 模板中、`case-default` 或 `match` 被局部绑定/定义/import 遮蔽、或外层宏可能读取该参数时，给出 `requires-review`，由人工改写为 `match` 或 `cond`。外层是保持参数原样求值的 core macro（如 `let`、`when`、`cond`）时直接通过；外层是其他宏时，只有宏源码对承载该调用的参数只做 `~` / `~@` 拼接（rest 参数另允许 `count` / `empty?`），且拼接位置外层都是函数、语法或同类 core macro 时才通过，例如 Respo 的 `defcomp`；`->` 等线程宏会改写参数位置，总是需要 review。先用 `calcit calcit.cirru fix --rule case-default-to-match-v1 --include-attached --format edn` 预览，核对 revision 后带 `--expect-revision` 应用。本规则暂不加入 preset；已知活跃下游默认分支的 `case-default` 清零后，与 `case-default` 一起在下一个非 patch 版本退场。
- `core-macro-alias-v1` 把已弃用的 core 便利宏改写为它们的展开结果：`w-log x` → `dbg x`（`w-log` 转发到 `dbg`，打印内容不变），`wo-log x` / `wo-js-log x` → `x`，`flipped f a b` → `f b a`。每条改写都是宏自身的展开，求值顺序与结果不变；嵌套的已证明调用合并在外层的一次替换中。`w-js-log` 交给 `js/console.log` 的是宿主对象而 `dbg` 打印格式化文本，只给 `requires-review`。调用位于 quasiquote 模板中、宏名或 `dbg` 被局部绑定/定义/import 遮蔽、参数个数与定义不符，或外层宏可能读取该参数时（判定方式与 `case-default-to-match-v1` 相同），给出 `requires-review`；外层是 `wo-log`、`wo-js-log`、`flipped` 或 `noted` 时视为原样转发，外层是 `w-log` / `dbg` 时因为打印的源码会改变而需要 review。`noted` 不在本规则内：`calcit query anchors` 读取它的 `@anchor:` 标注。用 `calcit calcit.cirru fix --rule core-macro-alias-v1 --include-attached --format edn` 预览，核对 revision 后带 `--expect-revision` 应用。本规则暂不加入 preset。
- `let-sugar-to-let-v1` 把已弃用的 `let-sugar` 改写为它的展开：按原顺序，连续的 symbol 绑定合成一个顺序求值的 `let`，每个 `([] ...)` 模式变成嵌套的 `let[]`，每个右侧仍只求值一次。嵌套的已证明调用合并在外层的一次替换中。`({} ...)` 模式依赖已弃用的 `let{}`，只给 `requires-review`，由人工改成 `let` 逐个绑定 `&map:get`；没有绑定或没有 body、调用位于 quasiquote 模板中、`let-sugar` / `let` / `let[]` 被遮蔽，或外层宏可能读取该参数时（判定方式与 `case-default-to-match-v1` 相同）同样需要 review。用 `calcit calcit.cirru fix --rule let-sugar-to-let-v1 --include-attached --format edn` 预览，核对 revision 后带 `--expect-revision` 应用。本规则暂不加入 preset；已知活跃下游默认分支的 `let-sugar` 清零后，与 `let-sugar` 一起在下一个非 patch 版本退场。
- `list-match-to-match-v1` 把已弃用的 `list-match xs (() empty...) ((head tail) body...)` 改写为 `match (destruct-list xs) ((:none) empty) ((:some head tail) body)`，保持分支顺序；分支有多个表达式时合成一个 `do`。两者都只求值 `xs` 一次并绑定相同的首元素与剩余 List。`destruct-list` 要求 `List<T>`，因此只有编译器已证明 `xs` 是 List（与 `calcit query type-at` 相同的证据）时才自动改写；Dynamic 或开放类型的 `xs` 给出 `requires-review`，需要先 decode 或收窄。分支形状不符、调用位于 quasiquote 模板中、`list-match` / `match` / `destruct-list` 被遮蔽，或外层宏可能读取该参数时（判定方式与 `case-default-to-match-v1` 相同，外层 `list-match` 视为原样转发）同样需要 review。用 `calcit calcit.cirru fix --rule list-match-to-match-v1 --include-attached --format edn` 预览，核对 revision 后带 `--expect-revision` 应用。本规则暂不加入 preset；已知活跃下游默认分支的 `list-match` 清零后，与 `list-match` 一起在下一个非 patch 版本退场。
- `apply-args-to-loop-v1` 把已弃用的 `apply-args (a b) (fn (x y) body...)` 改写为 `loop ((x a) (y b)) body...`：参数列表开头的 `[]` 按宏的规则去掉，两者按相同顺序求值实参并运行同一个 body，`recur` 仍然重启这个循环。callee 必须是字面量 `fn` 或 `defn`，参数都是普通 symbol 且个数与实参一致；`defn` 的 body 引用自身名字时（生成的循环函数不绑定这个名字）给出 `requires-review`。callee 是其他表达式、带 rest / 可选参数、调用位于 quasiquote 模板中、`apply-args` 或 `loop` 被遮蔽，或外层宏可能读取该参数时（判定方式与 `case-default-to-match-v1` 相同）同样需要 review。用 `calcit calcit.cirru fix --rule apply-args-to-loop-v1 --include-attached --format edn` 预览，核对 revision 后带 `--expect-revision` 应用。本规则暂不加入 preset；已知活跃下游默认分支的 `apply-args` 清零后，与 `apply-args` 一起在下一个非 patch 版本退场。
- `core-effect-method-v1` 只迁移已证明的 core nominal 效果方法：`FfiTask .cancel/.cancel-with` → `.cancel!/.cancel-with!`。`FsPath .write-text` 与 `FfiResponse .resolve/.reject` 已在 0.29.0 删除，需在升级 CLI 前用 0.28.x 的同名规则迁移。前缀 `task .cancel` 和紧凑 `task.cancel-with` 写法都可处理；必须同时证明接收者、core trait 来源、旧新方法签名及同一 helper，才能保持参数求值次数、Result/Unit 与宿主生命周期语义。开放接收者与用户自定义同名方法不自动改写，未知 macro 只给 `requires-review`，quoted data 跳过。先用 `calcit calcit.cirru fix --rule core-effect-method-v1 --format edn` 预览，再带相同 selector 和 `--expect-revision` 应用，重复预览应为空。本规则包含在 `core-api-0.28-v1`，旧 surface preset 不变，也不提供新的宿主能力。
- `core-list-add-v1` 仅把类型和方法契约均已证明的 List `.add` 改成 `.append`。它要求旧、新方法都指向
  `calcit.core/append`，形参和返回类型一致，且源码只经过已知保持调用的结构；Set/Map `.add` 不属于此规则，
  开放 List、未知 macro 和无法回溯的接收者只给 `requires-review`。显式运行
  `calcit calcit.cirru fix --rule core-list-add-v1 --format edn` 预览，再核对 definition、path、来源和 revision；
  应用后重复预览并运行项目测试。该规则也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-collection-len-v1` 仅把 List、Map、Set 上已证明的零参数 `.count` 改成 `.len`。
  编译器须同时证明具体接收者类型、旧/新方法指向同一个对应的 core `&*:count` 实现、形参与返回类型一致，
  且源码处于已知保持调用的结构。Struct 字段数、Enum payload 数、用户自定义 `Countable` 不在自动范围；
  开放类型与未知 macro 只给 `requires-review`，quoted data 不扫描。String `.count` 已在 0.29.0 删除
  （`E_RETIRED_METHOD`），其迁移需在升级 CLI 前用 0.28.x 的本规则完成；`.len` 同样按 Unicode 标量计数。用 `calcit calcit.cirru fix --rule core-collection-len-v1 --format edn` 预览，
  核对来源与 revision 后带 `--expect-revision` 应用；重复预览应为空，再运行严格检查与项目测试。
  该规则也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `rename-definition-v1` 是参数化语义重构规则。它要求 `--ns`、`--def` 与 `--to`，只改写 resolver 已证明指向
  同一项目 definition 的源码引用，并在同一事务中移除旧 `:refer`、重命名声明。裸引用会写成完整 namespace 路径，
  避免新名称被调用点的局部 binding 遮蔽；已有 `:as` 限定名会保留 alias。definition-attached tests 与 examples
  使用同一个 resolver trace，schema 则按 compiler-loaded nominal type reference 与 `:where` trait bound 改写；三者与普通 code、imports、声明在
  同一事务中提交。macro 生成引用、dependency source、quoted data 或缺少 source coordinate 的引用会拒绝整个事务。
- `rename-local-v1` 是参数化语义重构规则。它要求 `--ns`、`--def`、`--at <binding path>` 与 `--to`，`--at` 指向
  `defn`/`fn` 参数、`let`/`&let` 绑定名或 `loop` 绑定名。规则按 core 绑定形式计算作用域：`let` 按顺序绑定，`loop` 的初始值属于外层，
  内层同名绑定遮蔽的区域不会改写；每个改写的使用处还必须在预处理结果中解析为局部引用。新名字已在作用域中出现，使用处出现在
  quote/quasiquote、项目或依赖 macro 的参数，或会引入绑定的 core macro（如 `->%`、`if-let`、`let{}`）参数中时，整体拒绝并给出位置；
  `when`、`->`、`cond` 等不引入绑定的 core macro 参数照常改写。只改写当前定义的 code，不改 tests/examples。
  绑定本身写在 macro 调用里（如 Respo `defcomp` 的 body 中的 `let`）可以改名；`defcomp` 这类自定义定义 macro 的参数表不被识别为绑定。
- `value-to-zero-arg-fn-v1` 是参数化语义重构规则。它要求 `--ns` 与 `--def`，把精确的 `(def name value)` 改成
  `(defn name () value)`，把原 schema 包成零参数函数返回类型，并把 resolver 已证明的项目源码、attached tests 与 examples
  中的读取改成调用。它会改变求值时机：原值在 definition 初始化时求值一次，新函数则在每次调用时重新求值；因此只允许显式
  选择，不进入任何升级 preset。quoted data、macro 生成引用、dependency source、schema type reference、自引用或缺少 source
  coordinate 的引用会拒绝整个事务。
- `synthesize-schema-v1` 是参数化类型改写规则。它要求 `--ns` 与 `--def`，直接读取正常预处理及自底向上的类型推导结果，
  只填补源码 schema 中已有的 `Dynamic` 洞。零参数函数、可推导返回值和 `Ref<T>` 等候选只有独立实现与调用点证明通过，
  且没有剩余洞时才标记为 `machine-applicable`；缺少证明或局部参数、嵌套泛型等位置仍缺少约束时保留现有证据和精确路径，
  并标记为 `needs-review`，即使传入 `--apply`
  也不写回。该规则不进入升级 preset，不处理 macro、data/trait/impl 声明，也不通过执行程序猜运行时类型。
- `spread-call-proof-v1` 将有完整证明的 `f & ([] 1 2)` 改为 `f 1 2`，也支持 spread 前已有固定实参。
  它读取唯一的编译器 source-expression、真实 List 构造器和固定 callable 契约，按通用类型关系逐项证明，
  保留 head、实参的单次求值与顺序。预览先运行
  `calcit fix --rule spread-call-proof-v1 --ns app.main --def run --format edn`；review 后再使用现有
  `--apply --expect-revision <revision>`，写回仍经过 staged preprocess 和原子事务。该规则不进入默认 preset。
  已证明的固定参数加末尾 rest 调用保持原样：固定参数类型、spread 的 List 元素与 rest 合同一致时，
  变长调用本来就合法，不产生修复或待审建议，也不把它展开成固定 arity。
  注册的宿主 proc（如 `&call-dylib-edn`）只有描述符里的 arity，没有元素类型合同：spread 前的显式实参已满足最小 arity、
  proc 没有最大 arity 且 spread 实参静态为 List 时，同样视为合法变长调用；调用作为 macro 实参时按唯一的展开后表达式判断。
  其他候选的处理边界见末尾“限制”。
- `unsafe-coerce-boundary-v1` 把现有 `E_UNSCOPED_UNSAFE_COERCE` 转为可导航的待审建议，运行
  `calcit fix --rule unsafe-coerce-boundary-v1 --ns app.main --def run --format edn` 查看编译器位置、源码指纹、
  词法函数位置与编译栈。先核对捕获、效果和失败路径，再人工选择 checked decoder 或显式 adapter。
  建议的 replacement 为空，`--apply` 不修改源码或授予 `:js-ffi`。编译错误保留在 diagnostics 中，命令仍非零退出，
  不能用预览成功替代 strict 编译。已有明确词法权限的 adapter 不生成建议；词法权限以编译器规则为准，
  不因函数名、namespace 命名或调用者的权限推断独立 source definition 的权限。
  此规则须使用严格类型模式，不进入默认 preset 或 strict workflow 的自动改写集合；只检查 definition `:code`，
  每个 definition 在首个编译错误停止。修复后重新运行以发现后续错误；`:tests` / `:examples` 仍需独立验证。
  不相关错误、依赖或所选 scope 外的错误、缺少源码定位的错误直接失败，不猜定位或隐藏问题。
- `concrete-return-proof-v1` 复用同一 compiler proof，显式检查函数实现是否证明了声明的具体返回类型。
  运行 `calcit fix --rule concrete-return-proof-v1 --ns app.main --def run --format edn`。
  缺证据的 producer 保留 `E_FN_RETURN_UNPROVEN`、真实 source owner、path、fingerprint 与编译栈；
  wrapper 的返回声明不能代替 producer body 证明。所选 scope 不含真实 owner 时直接失败，扩大显式 scope 后重查。
  调用具体参数契约时，动态值或未知实参缺少证明会报告 `E_CALL_ARGUMENT_UNPROVEN`，不能借用 callee 的返回声明。
  字面量列表的 `&` 展开逐项复用参数证明；无法静态确定参数布局的展开保留为人工 review，已确定的类型矛盾以 `E_CALL_ARGUMENT_MISMATCH` 阻断审计。
  显式开放参数仍允许传递 Dynamic；decoder 成功分支提供的真实类型证据可以用于具体调用。
  独立证明的 identity 和显式 `Dynamic -> Dynamic` 不产生建议；矛盾返回或未解决的断言、强转边界阻止审计，不能被隐藏。
  短路 fold 使用 initial/default 的共同累积契约，并检查实际 reducer 的 Bool 控制位和各分支 payload。
  可读取的预处理函数体能提供证据；单独的 `Enum` 返回声明不证明这些槽位。开放 callback、未知 payload
  或无法追溯实现的值仍需 review；依赖函数自己的证明错误不会因为 fold 获得具体类型而被隐藏。
  replacement 为空，`--apply` 不修改代码或业务契约。只有 producer 实现可独立证明且缺少 schema 时，
  才另用现有 `synthesize-schema-v1` 审阅 metadata 候选；它仍执行独立证据、调用点一致性与原子写入门禁。
  此规则只审计 definition `:code`，须使用严格模式；strict workflow 已包含此检查，默认 preset 不变。
- `callable-contract-proof-v1` 检查具体调用所需的参数证明，运行
  `calcit fix --rule callable-contract-proof-v1 --ns app.main --def run --format edn`。
  裸 `Fn` 可以保存、返回和传递；传入具有具体参数/返回契约的 callback 位置时，必须由实现或真实边界提供证明。
  报告复用 `E_CALL_ARGUMENT_UNPROVEN`、源码位置与指纹；直接 callable 契约的 hint 展示签名、rest 和声明的 features，
  不把 `fn?` 当作签名验证，也不自动授予 features。未知外部 callback 保持待审，replacement 为空，`--apply` 不改源码。
  该入口运行共享 compiler proof，不另建 callable 检查器；同一定义较早的非 callable 参数缺证据也会先报告。
  矛盾类型、未解决的返回/断言错误或 scope 外错误会阻止审计。每个定义只定位首个错误，只扫描 `:code`；
  `:tests` 与 `:examples` 仍需另行验证。当前规则要求严格模式，不进入默认 preset 或 strict workflow 的自动写回集合。
- `nominal-write-proof-v1` 检查名义 Struct 的字段写入，运行
  `calcit fix --rule nominal-write-proof-v1 --ns app.main --def update --format edn`。
  `state .assoc :count incoming` 沿真实选中的方法实现读取字段契约；Number 字段接受已证明的 Number，
  Dynamic 输入保留 `E_CALL_ARGUMENT_UNPROVEN`，报告实际 value 位置及字段名，不自动插入强转或改成 Map。
  prefix/postfix 方法与底层写入复用同一证明关系；用户自定义实现不因方法同名而被当作 core 写入。
  明确类型矛盾或不存在的字段阻断审计，较早的其他 compiler 错误仍会先报告。
  建议只供 review，replacement 为空；只扫描 `:code`，不进入默认 preset 或 strict workflow 自动写回集合。
- `assert-type-proof-v1` 显式审计断言前的类型证据，运行
  `calcit fix --rule assert-type-proof-v1 --ns app.main --def run --format edn` 查看原始输入位置和编译栈。
  已知矛盾保留 `E_ASSERT_TYPE_MISMATCH`；开放输入、未知 callable 或未绑定泛型缺少证明时报告
  `E_ASSERT_TYPE_UNPROVEN`。审计在更新局部类型之前停止，因此不能通过再加一层断言消除待审边界。
  proof 错误的 `provenance` 给出局部绑定、编译器解析到的 producer 及可读取的返回表达式，
  方便从失败点追到 helper 的开放契约。它只解释已有错误，不提供新的类型证明或自动选择 decoder。
  同名局部绑定按词法作用域区分；来源最多展开 8 层、保留 16 项，不能将其当作完整运行时数据流。
  生成节点没有精确坐标时 path 为空；条件表达式的实参仅表示可能贡献，不表示运行时选中了该分支。
  当前预处理涉及的函数体还须独立证明其具体返回契约；仅靠返回声明的 producer 报告
  `E_FN_RETURN_UNPROVEN`，并定位其真实 source owner。Number identity、泛型 T identity 和明确的
  Dynamic 保存/传递可以保持原写法；未知 T 不能靠目标 Number 获得证明。
  当参数类型正是返回泛型 `'T` 时，`list?`、`map?`、`string?` 等类别谓词守卫的分支把 `'T` 视为该类别，
  分支出口按该类别证明；未被守卫的出口、作用于其他值的谓词和开放结果仍须证明原泛型契约；守卫分支若含 `set!` 重绑定局部，则放弃该收窄。
  producer 实现与返回声明矛盾时，既有 `W_FN_RETURN_TYPE_MISMATCH` 同样使审计失败，不能借用该声明获得空建议。
  将报告中的 definition 与 path 传给 `calcit query type-at <definition> --path <path> --format edn`
  可读取同一源码节点；展开后无法保留尾表达式坐标时，path 为声明根节点 `code`。
  规则复用 compiler 的共同证明关系与既有 suggestion，不运行程序猜类型，也不自动补 schema、选择 decoder、
  删除断言或插入强转。replacement 为空；`--apply` 不写入源码。报告保留 error，命令非零退出。
  此项审计须使用严格模式；strict workflow 已包含此检查，默认 preset 和普通编译的开放边界迁移策略保持不变。
  所选定义与其依赖都重新预处理，不把入口预检或普通编译的缓存当成审计证明；递归继续使用既有编译 guard。
  每个 definition 在首个错误停止，只检查 `:code`，`:tests` / `:examples` 需另行运行。
  所选 scope 外、未知来源及其他编译错误会直接失败，不能将空报告当作这些路径已安全的证明。
- `tag-match-to-match-v1` 与 `required-struct-field-v1` 属于只随 Calcit 0.14.15 发布的 migration bridge，不是当前 fix surface。
  升级旧项目时请固定使用 0.14.15 执行规则、review 输出并验证测试；迁移完成后再切换到 0.14.16 或更新版本。
  当前版本若显式请求这两个 rule，会返回稳定错误和上述版本提示，不会继续携带旧 planner 与分析特例。
  replacement 原样保留 receiver 子树且只出现一次，因此不会复制或重排求值。

## 运行版本化升级 preset

`surface-latest-v1` 是第一版冻结集合，按确定顺序展开为
`removed-data-api-v1`、`named-enum-constructor-v1`、`named-struct-constructor-v1` 和
`redundant-do-v1`，其含义保持不变。当前推荐的 `surface-latest-v2` 在这四条之后增加
`single-expression-do-v1`。执行时会按 source path 从内向外应用结构改写，具名构造器区域内的规则合并为一次 guarded replacement，
避免嵌套改写提前移动 source path。Cirru EDN preview 会在 `:data :filters :preset-id` 返回 preset 名称，并在
`:data :filters :expanded-rule-ids` 返回实际执行的规则，Agent 无需依赖人类日志猜测范围：

```bash
calcit calcit.cirru fix --preset surface-latest-v2 --format edn
calcit calcit.cirru fix --preset surface-latest-v2 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --preset surface-latest-v2 --format edn
```

`--preset` 与 `--rule` 互斥。apply 必须原样重复 preview 的 `--preset`、`--ns` 和 `--def`；第二次 preview
应返回空建议。未来集合发生变化时应发布新的 preset ID，既有 ID 不应静默改变含义。

### 0.28 核心 API 命名迁移

`core-api-0.29-v1` 包含 `core-api-0.28-v1` 的全部 5 条规则，并追加 `core-ref-constructor-v1`，共 6 条；从 0.28 升级的项目可以直接选择它，`core-api-0.28-v1` 的含义不变。

`core-api-0.28-v1` 从已发布的 `0.28.0-alpha.2` 起提供，组合已证明等价的 5 条叶子改写规则（0.29.0 起去掉了随旧名退役的 10 条别名规则，见 [升级指南](upgrade.md#兼容入口的退场节奏)）。它适合在结构升级后一次迁移核心 API 名称；每个调用仍须满足对应规则的类型、来源和源码位置证明。表中的规则也可用 `--rule` 单独选择，未回填进旧的 `surface-latest-v1/v2`；不要把“旧 preset 不变”理解为需要逐条运行所有规则。

| 范围 | 展开的规则 |
| --- | --- |
| 整数谓词与标量转换 | `core-integer-predicate-v1`、`core-identity-conversion-v1` |
| List 追加与集合长度 | `core-list-add-v1`、`core-collection-len-v1` |
| 效果方法 | `core-effect-method-v1` |

```bash
calcit calcit.cirru fix --preset core-api-0.28-v1 --format edn
calcit calcit.cirru fix --preset core-api-0.28-v1 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --preset core-api-0.28-v1 --format edn
```

此集合只替换 definition `:code` 内已证明的叶子，嵌套调用的参数和 source path 保持原位。预览的 `:data :filters :source-coverage` 在 `:scanned-regions` 列出字符串 `|code`，在 `:manual-review-regions` 列出 `|tests`、`|examples`。未证明的候选继续作为 `requires-review` 返回，quoted data 和 macro 定义沿用各规则的边界。普通函数、primitive 和方法参数的求值上下文由 reader 或 compiler-resolved 来源确认，未知宏仍需人工核对其是否观察源码拼写。

应用后重复预览，`machine-applicable` 建议应为空；`requires-review` 候选可能仍在，不能把它们当成已迁移或为了清零而自动应用。逐项核对这些候选以及 attached `:tests` / `:examples`，再运行项目严格检查和测试。使用已发布 `0.28.0-alpha.2` 的 Respo 副本验证中自动迁移 15 处后没有剩余可自动应用的建议，32 处人工审阅候选保留，48 个 definition tests 全部通过；这不代表整个项目的旧名已经清零。

先用 `surface-latest-v2` 整理构造器和 `do` 结构，再运行本 preset；如需迁移 `%some/%none/%ok/%err` 或 `option:*/result:*` helper，分别使用 `core-nominal-constructor-v1`、`core-option-method-v1`、`core-result-method-v1` 并逐次核对新 revision。这些规则会改写完整调用树，不能和叶子集合未经组合证明就一次应用。旧 `surface-latest-v1/v2` 的冻结范围保持原有定义；strict workflow 的证明检查另见下节。

单独迁移 Option/Result 构造 helper 时，先在真实项目中预览 `requires-review` 与 `machine-applicable`
的区别，再应用并重新预览确认幂等。应用前必须确认项目 `deps.cirru` 所锁定的 Calcit 版本与运行的 CLI 一致：

```bash
calcit calcit.cirru fix --rule core-nominal-constructor-v1 --format edn
calcit calcit.cirru fix --rule core-nominal-constructor-v1 --apply --expect-revision 'md5:<预览返回的 revision>'
calcit calcit.cirru fix --rule core-nominal-constructor-v1 --format edn
```

## 项目级严格迁移工作流

需要升级整个项目时，继续使用 `fix`，不要增加 `calcit migrate` 顶层入口：

```bash
calcit calcit.cirru fix --workflow strict --format edn
calcit calcit.cirru fix --workflow strict --apply \
  --expect-revision 'md5:<plan 返回的 revision>' --format edn
calcit calcit.cirru fix --workflow strict --verify --format edn
```

`:data :workflow` 是可保存、可重复生成的 `strict-v1` manifest。它按稳定顺序列出所有 entry 及 type slots、
`surface-latest-v2` 的安全建议、需要 review 的弱类型位置、已声明保留的动态边界、显式 FFI 边界、已有 verification profile，以及逐 entry
的严格检查命令。命令用 token list 表示，Agent 不需要重新解析 shell 字符串；`:resume :revision` 与
`:resume :apply-command` 给出恢复下一步所需的精确 revision guard。
workflow 还组合 `spread-call-proof-v1` 与 unsafe、assert、return、callable、nominal-write 的既有证明规则，
共用一次编译器证明过程，而不是逐条重复检查。可定位的问题进入原有 suggestions 和
`:review-required :source-fixes`；无法定位或来自依赖的错误保留在顶层 diagnostics，不猜测可写回位置。
存在证明错误时，preview/apply 状态为 `requires-review`，verify 为 `failed`，命令返回非零。
`--apply` 只应用已证明安全的源码迁移，仍需处理剩余证明错误；核对 `:safe-fixes` 与新 revision 后再继续。
显式开放的集合可以保存开放值，具体元素契约仍需在返回或使用处证明；macro 实现的语法构造与展开后的运行时代码分层检查。
每个定义只报告首个编译错误；`:tests` 与 `:examples` 不在此扫描范围，需另行执行。
`:preflight` 与 `analyze verify` 共享只读证据：Snapshot path/revision、Calcit/`@calcit/procs` 版本链、显式声明的 host
requirements，以及只由调用方执行的 external gates。它不会运行 shell gate，也不会从项目文件猜测包管理器。
无法自动应用的 source suggestion 单独保留在 `:review-required :source-fixes`，不会计入
`:safe-fixes :suggestions`。只有 unresolved、declared-optional 与 explicit-unsafe 类型位置进入
`:review-required :type-findings`；已显式声明的 JS FFI/type-slot Dynamic 单独进入 `:retained-type-boundaries`，
macro syntax 与明确的 Unit 返回不会伪装成迁移债务。

`:review-required :ffi-boundaries` 与
`analyze weak-types --ffi-evidence` 复用同一份静态证据：每个 operation 都有 host 分类和 source path，
并列出可静态找到的 caller、nullable/unsafe 路径、exact-schema helper 以及 trait/adapter 候选。helper 会扫描已加载的
模块并优先列出 dependency 候选；caller 仅承诺完全限定调用和同 namespace 直接调用，不冒充动态调用图。
所有候选都是审阅清单，不是可应用 fix；workflow 不据此推断外部值可信、返回值是否 optional，或生成业务默认值。

`:review-required :schema-candidates` 与 `analyze weak-types --schema-evidence` 复用 `synthesize-schema-v1` 的
编译器和 resolver call-site 证据；`:structural-candidates` 汇总重复匿名 Map shape 与 `match` tag dispatch。
候选携带 `exact`、`usage-derived`、`boundary-unknown` 或 `conflict`、源码路径和受影响调用点，但 workflow 不会
自动选择或应用它们。只有用户显式运行 `synthesize-schema-v1` 时，原有 machine-applicable schema fix 才进入写回流程；
Map/dispatch 仍只用于人工建模。

默认模式只规划。`--apply` 仍走既有 fingerprint、staged Snapshot、严格预处理和原子替换，不会应用 schema 设计、
Dynamic 收窄、FFI trust 或业务默认值。`--verify` 是只读模式：它要求安全建议已经清空，逐 entry 运行
`--check-only --keep-going`，并执行 Snapshot 已声明的 verification profiles；任一结果失败时保持结构化 stdout，
同时返回非零退出码。`:external-commands` 保持为空，因为 Calcit 不猜测项目使用 yarn、npm、cargo 或其他构建器；
调用方应把外部构建作为显式步骤维护，并可用 Snapshot `:verification :external-gates` 把步骤名称带入 preflight。

`--workflow strict` 是项目级组合视图，因此与 `--ns`、`--def`、`--rule`、`--preset` 和 `--to` 互斥；
`--verify` 与写入选项互斥。Cirru EDN 是 manifest 的首选格式，只有 JSON-only consumer 才显式选择 JSON。

## 局部绑定改名

`tree search-replace` 按文本匹配叶子，分不清绑定处、使用处和同名遮蔽。改局部名时先用 `query def` 或
`query search` 找到绑定名的路径，再走 preview/apply：

```bash
calcit calcit.cirru fix --rule rename-local-v1 \
  --ns app.comp --def comp-task --at 3.1.3.0 --to text --format edn
calcit calcit.cirru fix --rule rename-local-v1 \
  --ns app.comp --def comp-task --at 3.1.3.0 --to text \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
```

应用后再次预览为空。

## 原子重命名 definition 与静态引用

声明和调用点需要一起变化时，不要先执行 `edit rename` 再文本搜索。使用同一个 `fix` preview/apply 闭环：

```bash
calcit calcit.cirru fix --rule rename-definition-v1 \
  --ns app.core --def old-name --to new-name --format edn
calcit calcit.cirru fix --rule rename-definition-v1 \
  --ns app.core --def old-name --to new-name \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --rule rename-definition-v1 \
  --ns app.core --def old-name --to new-name --format edn
```

preview 的每个 usage suggestion 都包含 resolved target 的 `:origin-chain`、source path、fingerprint 和 quoted AST。
规划阶段会遍历普通 code、definition-attached tests、examples 与 schema，但不会把局部同名 binding 或同名依赖定义
当成 usage。tests 和 examples 通过 synthetic lexical scope 复用 compiler resolver；schema 只改写 loader 已保留并能解析到
目标 definition 的 nominal type reference 或 trait bound。测试名称/tags、example 顺序以及无关 schema 保持不变。包含目标名称的 quoted data
不作为可改写 usage，而是按潜在动态消费边界拒绝事务。staged Snapshot
会重新严格预处理整个项目；解析失败、旧引用残留或目标冲突时，原文件保持不变。重复执行原命令时，如果旧 definition
已不存在且新 definition 存在，会返回空 suggestions，作为幂等完成状态。

macro source、macro expansion、quote/quasiquote、dependency-owned source 以及无法映射回 Snapshot 的引用仍然 fail closed，
并列出 blocker；不得把这些边界降级为文本替换。
只想改声明、并明确自行负责调用点时，仍可直接使用 `calcit edit rename`。

## 把 value definition 重构为零参数函数

需要把延迟计算、环境读取或工厂值从普通 definition 改成显式调用时，使用 resolver 驱动的原子重构，不要先改 `defn` 再文本搜索：

```bash
calcit calcit.cirru fix --rule value-to-zero-arg-fn-v1 \
  --ns app.config --def current-config --format edn
calcit calcit.cirru fix --rule value-to-zero-arg-fn-v1 \
  --ns app.config --def current-config \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --rule value-to-zero-arg-fn-v1 \
  --ns app.config --def current-config --format edn
```

例如普通读取 `current-config` 会变成 `(current-config)`；函数值原来作为 callee 的
`stored-handler event` 会变成 `(stored-handler) event`。目标 definition 的原 schema 会成为新零参数函数的返回类型。
普通 code、attached tests、examples、schema 与 definition replacement 在一个 staged transaction 中验证并提交；重复 preview
应为空。

这条规则有意改变生命周期，不承诺语义等价：副作用、时间、随机数、环境变量、对象身份、分配成本以及原先隐含的缓存都会从
“初始化一次”变成“每次调用”。preview 虽能证明引用归属和改写完整性，不能替用户决定这种变化是否正确。应用前必须审阅目标
value expression 和全部 suggestions；如果目标应继续只计算一次，就保留 `def`，不要运行这条规则。

该规则不会改写 quoted data，也不会猜测 macro 展开、动态名称查找、dependency source、把目标当成类型使用的 schema 或
自引用初始化。命中任一边界时整个事务 fail closed，原文件不写入。由于它是用户选择的语义重构而非版本迁移，当前及未来 preset
都不应隐式包含它。

## 从现有推导事实合成 schema

缺失 schema 时先查询单个 definition，不要直接把整个签名填成 `Dynamic`：

```bash
calcit calcit.cirru fix --rule synthesize-schema-v1 \
  --ns app.model --def initial-count --format edn
calcit calcit.cirru fix --rule synthesize-schema-v1 \
  --ns app.model --def initial-count \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --rule synthesize-schema-v1 \
  --ns app.model --def initial-count --format edn
```

规则复用编译器已经产生的 compiled form、局部类型元数据与 bottom-up inference，不维护另一套递归类型模型。
例如 `defatom *count 0` 可以产生 `Ref<Number>`，无参数且返回数值表达式的函数可以产生 `Fn() -> Number`；已有
`Fn(...)->Dynamic`、`Ref<Dynamic>` 或嵌套容器只替换能够由同形状推导结果证明的洞，声明中已有的 generics、`:where`、
features、参数与函数类别保持不变。

结构化 suggestion 的 `:origin-chain` 会给出 compiled inference 候选以及仍未解决的 slot path。只有
`:applicability :machine-applicable`、独立实现证明通过且未留下 `Dynamic`/未知 callable 的候选会生成事务 operation。`needs-review`
候选只展示更精确的外层结构，例如 `Option<Dynamic>`，不会把未知 payload 扩大成整个 `Dynamic`，也不会在 `--apply`
时部分写回。无法从静态实现恢复类型时命令明确失败；macro、nominal data、trait 和 impl contract 必须继续显式声明。

候选没有剩余洞也可能需要 review：函数返回声明和未经证明的 `assert-type` 不能为自己提供证据，
`unsafe-coerce` 的目标类型只是显式信任契约。证明审计检查强转前的实际类型；真实 decoder 的成功结果和已证明的
断言仍可支持补全。`:origin-chain` 中保留编译器诊断及原始位置，未证明候选即使传 `--apply` 也不写回。
只读 `analyze weak-types --schema-evidence` 保留这些候选为 `boundary-unknown`，其他定义继续正常查询。

实现侧证据可补全 value、zero-argument function、`Ref<T>` 与函数返回洞。带参数函数还会遍历普通项目源码中 resolver
确认的调用点；只有调用形态完整、没有 macro/函数值等动态边界，并且同一参数的所有可推导实参类型完全一致时，才把该类型作为
当前 definition graph 的参数约束。缺失、冲突或无法定位的调用证据会保留 `schema.args.<index>`。namespace 名称中以独立点分段出现
`test`、`tests`、`example` 或 `examples` 的普通源码，以及 definition-attached tests/examples，都只作为样本，不作为公共签名证明；
但候选进入 staged transaction 后，引用目标的 attached tests/examples 仍必须在新 schema 下通过严格预处理，
否则整项拒绝且不写回。native 与 JS entry 使用同一预处理事实并应得到相同候选。

具名构造器规则只处理原型能静态解析到当前项目 nominal definition 的直接源码。匿名 `%:: _` / `%{} _`、
运行时 prototype、依赖中无法回溯的定义、局部同名遮蔽、`defmacro` 以及 `quote`/`quasiquote` 内的数据都会保留。
Struct 规则还要求旧字段是完整的 `(:tag value)` pair；动态字段集合不做猜测。

## 检测并修复冗余 `do`

`defn`、`fn` 和 `let` 的 body 本身支持多个顺序表达式，并以最后一项作为结果。下面的外层 `do` 不补充
类型推断或求值能力，只增加 AST 层级：

```cirru.no-check
defn render! (state)
  do
    println |rendering
    view state
```

`redundant-do-v1` 会把它安全整理为：

```cirru.no-check
defn render! (state)
  println |rendering
  view state
```

先用 preview 检测，不要直接 apply：

```bash
calcit calcit.cirru fix --rule redundant-do-v1 --format edn
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format edn
```

Cirru EDN 报告中的 `:data :suggestions` 是检测结果。若列表非空，逐项核对 `:definition`、`:path`、`:original`、`:replacement`、
`:applicability` 和 `:fingerprint`，确认 `:data :validation :status` 为 `passed`，再原样重复 preview 的 scope selectors，
使用同一份报告中的 Snapshot revision 应用。若列表为空，`:data :validation :status` 应为 `not-needed`；跳过 apply，
直接继续验证：

```bash
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --ns app.main --def render! --rule redundant-do-v1 --format edn
```

第二次 preview 应返回空 `suggestions`，证明规则幂等。随后运行目标 entry 的 `--check-only` 和行为测试。

`redundant-do-v1` 只 splice 已知 variadic body 的直接 `do` 子节点。多表达式 `do` 在以下位置会保留：

- 只接收单表达式的位置（`if`/`case` 分支、函数调用参数、`binding value` 等）；
- `defmacro` body，因为 macro 可能需要显式打包返回的语法树；
- `quote`/`quasiquote` 内作为数据保存的代码。

`defn`、`fn`、`let`、`let[]` body 顶层的多余 `do` 都属可自动整理范围；JS FFI 适配器代码里常见的
`defn`/`let` 顶层 `do` 因此可以直接用这条规则收敛。`calcit fix` 的默认预览（不传 `--rule`/`--preset`
时）已包含 `redundant-do-v1`，报告中每项 suggestion 的 `diagnostic_code` 为稳定的 `FIX_REDUNDANT_DO`；
CI 可以在不写回 Snapshot 的前提下解析该报告并据此阻断。

`single-expression-do-v1` 则处理这些普通可执行位置中的 `(do expr)`，因为它没有第二个步骤；同样跳过
`defmacro` 与 `quote`/`quasiquote`。若一个项目同时需要两种整理，直接使用 `surface-latest-v2`，不要靠文本搜索判断层级。

## JS FFI 边界迁移（review-only）

`calcit fix --workflow strict` 在 `review_required.ffi_boundaries` 中携带 `--ffi-evidence` 的盘点结果：
每个裸 `JsObject` 访问会按 definition 分组，并给出 `operations`（kind/member/path）、callers 与 trait
candidate。每个 trait candidate 还附带一段可直接粘贴的 `defexternal` 骨架：

```cirru.no-check
defexternal RawReadHost
  :length 'Dynamic
```

骨架只包含能表达为 Calcit trait 成员的字段/方法名（索引、字符串 key 会被过滤），未知 target 不写
`:target`，字段/方法类型用 `Dynamic` 占位，`contract_status` 保持 `review-required`。它**不会自动
apply**：trait 名字、payload 类型与宿主名映射无法从成员访问唯一推断，自动改写会生成错误契约。正确流程是：

1. `calcit analyze weak-types --ffi-evidence --format edn` 或 `--warn-dyn-method` 盘点站点；
2. 参考骨架，用 `calcit edit def` 补全类型并建立 `defexternal` 契约；
3. 把访问收敛到边界 coercion 后的 typed 调用，再跑目标 entry 的 `--check-only` 与行为测试。

这与「自动改写边界」的原则一致：只有能唯一回到 source AST、证明保持求值与失败语义、并携带
revision/fingerprint 前置条件的建议才进入可 apply 的 preset。

## 自动改写边界

当前 preset 只收录能证明保持表层语义的一对一规则。以下项目继续由严格诊断定位，不能自动加入 preset：

- 已知 Struct 字段访问后的 `.unwrap`：必须先证明 receiver 与调用链，不能全局删除；
- `?` 参数、`Optional`、裸 `nil` 与 `%{}?`：需要选择 Option、Result、Unit 以及新的调用契约；
- Dynamic 收窄、raw primitive 与 `unsafe-coerce`：需要设计 schema 和 capability 边界；
- `get-or`、`first-or` 等到 `.unwrap-or`：需要 fallback 类型和缺失语义证据，后续按独立规则评估；
- Snapshot schema 与 canonical serialization：继续由 `calcit edit format` 负责；
- `tag-match-to-match-v1` 与 `required-struct-field-v1`：继续固定在已发布的 0.14.15 bridge。

因此“检测冗余 `do`”应使用 fix preview，而不是正则搜索，也不会作为普通 compiler warning 混入类型诊断。

Cirru EDN stdout 是一个完整 value，顶层包含 `:schema-version`、`:command`、Snapshot `:revision`、`:data`、`:diagnostics`
和 `:next`；`:data` 内包含 `:filters`、`:validation` 与 `:suggestions`。每条 suggestion 携带 Snapshot `:source-file`、stable rule ID、diagnostic code、表层 definition/path、subtree fingerprint、
quoted AST 的 original/replacement 与 applicability。结构化 splice 的 replacement 使用 `{} (:$type |splice) (:value $ [])`，
明确表示多个 sibling，而不是伪装成单个表达式。`validation` 说明 staged scope preprocess 是否执行并通过，以及实际检查的
operation 数量；没有可应用 operation 时状态为 `not-needed`。日志和 command echo 只写 stderr，Agent 不需要解析人类文本或生成的 JS。
只有对接 JSON-only consumer 时才显式使用 `--format json`；它与 Cirru EDN envelope 语义等价。

`:data :filters :expanded-rules` 进一步说明每条实际规则的 `:evidence-source`、`:diagnostic-code`、`:lifecycle` 和
`:source-version-required`。当前规则只从当前诊断或 resolved source AST 派生，`:source-version-required` 固定为 `false`；
稳定 rule ID 的版本号只表示协议行为，不表示待迁移项目的来源版本。

确认预览后再应用：

```bash
calcit calcit.cirru fix --ns app.main --def render! --apply --expect-revision 'md5:...'
```

apply 必须原样重复 preview 使用的 `--ns`、`--def` 以及 `--rule` 或 `--preset` selectors。revision 只证明 Snapshot 没有变化，不能证明
更大 scope 中的其他建议也经过审阅；省略 selectors 会重新规划整个项目，而不是只应用上一次预览的子集。

写入流程复用 `tree replace` 的 `--expect` guard 和现有 transaction：规划完成后即使调用方没有显式传
`--expect-revision`，transaction 也必须绑定规划时捕获的 revision。全部 replacement 先在 staged Snapshot 上执行，重新加载并
以调用方选中的同一 `--entry` 加载 modules、type slots 并预处理选定 scope，最后才原子替换源文件。带 `--ns` 的局部 scope
检查该 entry 的 target；不带 `--ns` 的整项目 staged 预处理与预览一样为 target 中立，不能证明 browser 或 Node 入口兼容，
仍需对每个 entry 分别运行 `--check-only` 或 `fix --workflow strict --verify`。revision 过期、节点不匹配、替换重叠、
parse/schema/preprocess 失败时均不写入。
代码改写不会顺带规范化未选中的 schema：只要其加载后的类型契约未改变，事务保留原来的 Cirru EDN 表层写法；
显式 schema 改写仍按预览列出的操作执行，整份 Snapshot 的格式升级继续单独使用 `calcit edit format`。
重复运行同一规则必须返回空 suggestions，不能再次改写。

为了让旧兼容代码在已经启用严格错误的版本中仍可迁移，初次规划会在隔离的兼容 preprocess 中收集 warning 和类型证据；这一步
只产生候选计划。staged Snapshot 应用 replacement 后会重新启动严格 preprocess，严格错误或新增无关 warning 仍会拒绝 preview/apply，
正常的 check、test、JS/WASM codegen 也不会继承规划模式。因此该机制不是降低类型门禁，而是让自动 migration 能先修复门禁指出的旧语义。

为降低 Agent 误写成本，`--apply` 默认要求干净的 Git worktree。确实需要在已有修改上应用时，先审阅 Cirru EDN preview，
再显式加 `--allow-dirty`；非 Git 环境需要显式加 `--allow-no-vcs`。这两个参数只放宽版本控制前置条件，不跳过 revision、
fingerprint、staged preprocess 或原子写入检查。

`calcit fix` 不会自动加入业务默认值、改变错误处理策略、插入 `unsafe-coerce`、扩大 `Dynamic`，也不会把 compiler-owned
core lowering 回写成表层源码。无法证明等价或无法唯一回溯 source origin 的建议保持只读，交给人类决定。

## 限制

- `spread-call-proof-v1` 对未证明的未知长度、开放实参、可选固定参数或 trait bounds 返回 `requires-review`，不生成 replacement，也不因 `--apply` 写入。
- 多重 spread 或不唯一的 macro/source 映射保持 `requires-review`，不自动改写。
- quoted data 和函数参数声明不作为 spread 调用检查。
