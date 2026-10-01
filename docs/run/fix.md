---
title: "Compiler-guided Source Fixes"
summary: "使用 calcit fix 预览并原子应用带 revision、fingerprint 与 quoted AST 的编译器指导型迁移"
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

`calcit fix` 把编译器已经能够唯一判断的迁移建议映射回 Snapshot source AST。默认命令只做预览；它会在同目录的
staged Snapshot 上尝试替换并重新编译选定 scope，成功后仍不写入原文件：

```bash
calcit calcit.cirru fix
calcit calcit.cirru fix --ns app.main --def render! --format edn
```

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
  `Result :ok value`。
- `named-struct-constructor-v1` 把能静态解析到项目 `defstruct` 的 `%{} Person (:name name)` 改为
  `Person :name name`，并保持字段表达式的原始求值顺序。
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
  也不把内部 helper 立即删除。与构造器规则一样，改写只覆盖定义的 `:code`，不盲改
  `:tests` / `:examples`。先预览、核对来源和 revision，再应用并运行严格检查与业务测试。
- `core-result-method-v1` 把 `result:ok?` / `result:err?` 改为 `value .ok?` / `value .err?`，
  也处理 `result:unwrap-or result fallback` 到 `result .unwrap-or fallback`；仍须证明 core 来源和
  同一个接收者方法契约，不因谓词返回 Bool 就跳过接收者类型检查。
  已证明来自 core 的 `%err error` 或 `Result :err error` 没有成功值，因而可由具体 fallback
  确定成功类型，同时保留错误类型；方法契约仍须为 `proven`，源码引用、求值次数和顺序也须可证明。
  成功值有具体类型、方法契约已证明的 core `%ok` 也可迁移；备用值不能代替成功值的类型证据。
  普通 `Result<Dynamic,E>`、成功值来自 Dynamic 的 `%ok` / `Result :ok`、同名类型遮蔽、
  函数值和未知 macro 只给 `requires-review`；具名构造若缺少精确源码类型证据同样保留审阅。
  与 Option 规则一样，本规则须显式选择，不修改已发布 preset，也不自动改写 `:tests` / `:examples`。
  先运行 `calcit calcit.cirru fix --rule core-result-method-v1 --format edn` 预览，再带原样
  `--expect-revision` 应用；重复预览应为空，随后运行严格类型检查和项目测试。
  0.26.0 不删除 Option/Result 旧方法 helper；它们仍是 core method 的实现目标。应用可直接调用的
  兼容入口最早于 0.27.0 且满足[退场条件](../features/api-roles.md#旧方法-helper-的退场条件)后才考虑移除，
  不能把一次空预览误认为已完成消费者迁移。
- `core-non-nil-predicate-v1` 把编译器已解析到 `calcit.core/some?` 的源码引用改为
  `calcit.core/non-nil?`。这是等价的非 nil 谓词改名，不会猜成 Option `.some?`，也不会改写用户自定义同名函数。
  规则保留实参位置、求值次数和失败行为；明确限定新引用可避免局部变量或 import 遮蔽。已知保持调用的 core macro
  可以自动迁移，未知 macro 只返回 `requires-review`。当前只扫描所选 definition 的源码 `:code`，不会盲改 attached
  `:tests` / `:examples`；这些位置应由测试和人工检查同步迁移。本规则也包含在新的 `core-api-0.28-v1`；已发布的 surface preset 保持原定义。
- `core-integer-predicate-v1` 把 Cirru reader 直接解析为内建 Proc 的单参数 `round?` 调用头改为
  `calcit.core/integer?`。新入口复用有限且恰好没有小数部分的既有语义；不会把 Bool 当作整数类型 refinement。仅改写可回溯的源码调用，保留实参原位与求值次数；含同名词法绑定的定义及 quoted 数据跳过，未知 macro 只给 `requires-review`。
  同一规则还会把静态 Number 接收者的 `.round?` 改成 `.integer?`：旧、新方法现均指向 `calcit.core/integer?`，类型契约同为 `Number -> Bool`；自定义同名方法不改，开放接收者与未知宏不会自动改写。当前只覆盖 definition `:code`，不自动修改一等函数引用或 attached `:tests` / `:examples`；这些位置需人工审阅。使用 `calcit calcit.cirru fix --rule core-integer-predicate-v1 --format edn` 预览，核对来源与 revision 后应用并重复预览；包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-identity-conversion-v1` 把参数已证明为 String 的内建 `turn-tag` / `turn-symbol` 完整调用分别改为 `calcit.core/to-tag` / `calcit.core/to-symbol`；也把解析到 core 兼容函数、且参数已证明为内建 Nil、Bool、Number、String、Tag 或 Symbol 的 `turn-string` 改为 `calcit.core/to-string`。这些标量路径调用相同的底层转换，实参仍只求值一次；自定义 `ToString` 实现、Dynamic、已知不支持的集合、局部同名绑定、quoted 数据、未知 macro 与一等函数值都不自动改写。仅扫描 definition `:code`，`:tests` / `:examples` 需人工检查。先用 `calcit calcit.cirru fix --rule core-identity-conversion-v1 --format edn` 预览，核对来源与 revision，再带 `--expect-revision` 应用并复查；包含在 `core-api-0.28-v1`；旧 surface preset 不变。WASM 仍不支持动态 Tag/Symbol intern，迁移不意味着获得 WASM 支持。
- `core-predicate-method-v1` 按已证明的接收者类型迁移谓词方法：List/String `.contains?` → `.contains-index?`，Map `.contains?` → `.contains-key?`、`.includes?` → `.contains-value?`，Set `.contains?` → `.includes?`。每一项都须证明旧、新方法解析到同一个 core 实现，参数与返回类型相同，且源码可稳定定位；不会把索引、键、值和成员混成一个命题。自定义同名方法不改，开放接收者不会自动改写，未知 macro 只给 `requires-review`，quoted data 跳过。先运行 `calcit calcit.cirru fix --rule core-predicate-method-v1 --format edn` 预览，再核对来源和 revision、带 `--expect-revision` 应用，最后重复预览并运行项目测试。当前只扫描 definition `:code`，不自动改 attached `:tests` / `:examples`、Struct/Enum、trait-bound 或具名 trait-call；本规则可单独选择，也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-effect-method-v1` 只迁移已证明的 core nominal 效果方法：`FsPath .write-text` → `.write-text!`，`FfiTask .cancel/.cancel-with` → `.cancel!/.cancel-with!`，`FfiResponse .resolve/.reject` → `.resolve!/.reject!`。前缀 `path .write-text` 和紧凑 `task.cancel-with` 写法都可处理；必须同时证明接收者、core trait 来源、旧新方法签名及同一 helper，才能保持参数求值次数、Result/Unit 与宿主生命周期语义。开放接收者与用户自定义同名方法不自动改写，未知 macro 只给 `requires-review`，quoted data 跳过。先用 `calcit calcit.cirru fix --rule core-effect-method-v1 --format edn` 预览，再带相同 selector 和 `--expect-revision` 应用，重复预览应为空。当前只修改 definition `:code`，`:tests` / `:examples` 需人工核对；本规则包含在 `core-api-0.28-v1`，旧 surface preset 不变，也不提供新的宿主能力。
- `core-list-add-v1` 仅把类型和方法契约均已证明的 List `.add` 改成 `.append`。它要求旧、新方法都指向
  `calcit.core/append`，形参和返回类型一致，且源码只经过已知保持调用的结构；Set/Map `.add` 不属于此规则，
  开放 List、未知 macro 和无法回溯的接收者只给 `requires-review`。显式运行
  `calcit calcit.cirru fix --rule core-list-add-v1 --format edn` 预览，再核对 definition、path、来源和 revision；
  应用后重复预览并运行项目测试。该规则目前只覆盖 definition `:code`，`:tests` / `:examples` 仍需单独检查，
  也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-set-include-v1` 仅把已证明的 Set `.add item` 改成 `.include item`（也支持原调用中的更多成员参数）。旧新方法都须解析到 `calcit.core/include`，并具有相同参数、返回类型与稳定源码位置；重复成员仍去重，返回新 Set。Map `.add` 接受二元 entry，List `.add` 表示追加元素，均不属于本规则；自定义同名方法、开放接收者、quoted data 与未知 macro 不自动改写。运行 `calcit calcit.cirru fix --rule core-set-include-v1 --format edn` 预览，核对来源与 revision 后携带 `--expect-revision` 应用，再重复预览并运行项目测试。当前仅修改 definition `:code`，`:tests` / `:examples` 人工核对；规则包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-collection-len-v1` 仅把 List、Map、Set、String 上已证明的零参数 `.count` 改成 `.len`。
  编译器须同时证明具体接收者类型、旧/新方法指向同一个对应的 core `&*:count` 实现、形参与返回类型一致，
  且源码处于已知保持调用的结构。Struct 字段数、Enum payload 数、用户自定义 `Countable` 不在自动范围；
  开放类型与未知 macro 只给 `requires-review`，quoted data 不扫描。String 两种方法都按 Unicode 标量计数，
  不会转为 UTF-8 字节长度。用 `calcit calcit.cirru fix --rule core-collection-len-v1 --format edn` 预览，
  核对来源与 revision 后带 `--expect-revision` 应用；重复预览应为空，再运行严格检查与项目测试。
  当前仅覆盖 definition `:code`，`:tests` / `:examples` 需单独检查；该规则也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-list-fold-v1` 仅把具体 List 上已证明的 seeded `.reduce` 方法改成 `.fold`。两者必须指向同一个
  `calcit.core/fold` 实现，参数与返回类型一致；空列表仍返回初值，按从左到右顺序调用 reducer，累加器类型可与元素类型不同。
  开放接收者、用户自定义同名方法、未知 macro 和 quoted data 不会自动改写。前缀函数 `reduce` 暂保留兼容，
  不在本规则范围。运行 `calcit calcit.cirru fix --rule core-list-fold-v1 --format edn` 预览，审阅来源与 revision
  后带 `--expect-revision` 应用，重复预览应为空。本规则仅覆盖 definition `:code`，`:tests` / `:examples`
  需单独检查；也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-list-intersperse-v1` 仅把具体 List 上已证明的 `.join separator` 改成 `.intersperse separator`。
  两者必须同指 `calcit.core/intersperse`，形参与返回契约一致。该操作返回 List，只在元素之间插入同类型分隔值；
  空 List、单元素、重复值和顺序不变。前缀 `join` 与返回 String 的 `join-str` 不是这条方法迁移的对象。
  运行 `calcit calcit.cirru fix --rule core-list-intersperse-v1 --format edn` 预览，核对来源和 revision 后携带
  `--expect-revision` 应用并重复预览。未知 macro、quoted data 和未证明的接收者不自动改写；当前只覆盖
  definition `:code`，不改 `:tests` / `:examples`，也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-list-join-string-v1` 仅把具体 List 上已证明的 `.join-str separator` 改成 `.join-string separator`。
  两者必须同指 `calcit.core/join-str`，形参与返回类型契约一致；保留对 List 元素的原有显示转换、String 分隔符
  和空 List 的空字符串结果。运行 `calcit calcit.cirru fix --rule core-list-join-string-v1 --format edn` 预览，
  核对来源与 revision 后携带 `--expect-revision` 应用并重复预览。前缀 `join-str`、quoted data、开放接收者、
  未知 macro 和用户方法不自动改写；当前只覆盖 definition `:code`，不改 `:tests` / `:examples`，包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-map-distinct-values-v1` 仅把具体 Map 上已证明的零参数 `.values` 改成 `.distinct-values`。
  两者必须同指 `calcit.core/distinct-values`，形参与返回契约一致，均返回去重的 Set，不能解释成保留重复值的 List。
  运行 `calcit calcit.cirru fix --rule core-map-distinct-values-v1 --format edn` 预览，核对来源与 revision 后携带
  `--expect-revision` 应用并再次预览。前缀 `vals`、quoted data、开放接收者、未知 macro 与用户自定义方法不自动改写；
  当前只覆盖 definition `:code`，不改 `:tests` / `:examples`，也包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-list-flat-map-v1` 仅把具体 List 上已证明的 `.bind callback` 改成 `.flat-map callback`。
  两者必须同指 `calcit.core/mapcat`，回调及返回类型契约一致；每个元素调用一次回调，将返回的 List 按顺序展平一层。
  运行 `calcit calcit.cirru fix --rule core-list-flat-map-v1 --format edn` 预览，核对来源和 revision 后携带
  `--expect-revision` 应用并重复预览。Fn `.bind` 是不同语义，开放接收者、未知 macro、quoted data 和用户方法
  不自动改写。当前只覆盖 definition `:code`，不改 `:tests` / `:examples`，包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-list-get-v1` 仅把具体 List 上已证明的 `.nth index` 改成 `.get index`。
  两者必须同指 `calcit.core/get`，形参与 `Option<T>` 返回契约一致；空 List 或越界仍返回 `none`。
  运行 `calcit calcit.cirru fix --rule core-list-get-v1 --format edn` 预览，核对 receiver、来源与 revision 后携带
  `--expect-revision` 应用并再次预览。String/Enum 的 `.nth`、前缀函数、开放接收者、quoted data、未知 macro
  和用户自定义方法不自动改写。当前只覆盖 definition `:code`，不改 `:tests` / `:examples`，包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `core-collection-combine-v1` 仅把具体 Map 的 `.mappend` 改成 `.merge`、具体 Set 的 `.mappend` 改成 `.union`。
  两组旧/新方法必须各自指向同一个 core 实现，形参与返回契约一致；Map 后出现的同名 key 覆盖前值，Set 去重。
  规则支持至少一个组合参数的完整方法调用，不处理作为值传递的方法名。运行
  `calcit calcit.cirru fix --rule core-collection-combine-v1 --format edn` 预览，核对 receiver、来源与 revision 后
  携带 `--expect-revision` 应用并再次预览。List、String、Fn 的 `.mappend` 语义不同，开放接收者、quoted data、
  未知 macro 和用户自定义方法也不自动改写。当前只覆盖 definition `:code`，不改 `:tests` / `:examples`，
  包含在 `core-api-0.28-v1`；旧 surface preset 不变。
- `rename-definition-v1` 是参数化语义重构规则。它要求 `--ns`、`--def` 与 `--to`，只改写 resolver 已证明指向
  同一项目 definition 的源码引用，并在同一事务中移除旧 `:refer`、重命名声明。裸引用会写成完整 namespace 路径，
  避免新名称被调用点的局部 binding 遮蔽；已有 `:as` 限定名会保留 alias。definition-attached tests 与 examples
  使用同一个 resolver trace，schema 则按 compiler-loaded nominal type reference 与 `:where` trait bound 改写；三者与普通 code、imports、声明在
  同一事务中提交。macro 生成引用、dependency source、quoted data 或缺少 source coordinate 的引用会拒绝整个事务。
- `value-to-zero-arg-fn-v1` 是参数化语义重构规则。它要求 `--ns` 与 `--def`，把精确的 `(def name value)` 改成
  `(defn name () value)`，把原 schema 包成零参数函数返回类型，并把 resolver 已证明的项目源码、attached tests 与 examples
  中的读取改成调用。它会改变求值时机：原值在 definition 初始化时求值一次，新函数则在每次调用时重新求值；因此只允许显式
  选择，不进入任何升级 preset。quoted data、macro 生成引用、dependency source、schema type reference、自引用或缺少 source
  coordinate 的引用会拒绝整个事务。
- `synthesize-schema-v1` 是参数化类型改写规则。它要求 `--ns` 与 `--def`，直接读取正常预处理及自底向上的类型推导结果，
  只填补源码 schema 中已有的 `Dynamic` 洞。零参数函数、可推导返回值和 `Ref<T>` 等候选没有剩余洞时标记为
  `machine-applicable`；局部参数、嵌套泛型等位置仍缺少约束时保留精确路径并标记为 `needs-review`，即使传入 `--apply`
  也不写回。该规则不进入升级 preset，不处理 macro、data/trait/impl 声明，也不通过执行程序猜运行时类型。
- `spread-call-proof-v1` 将有完整证明的 `f & ([] 1 2)` 改为 `f 1 2`，也支持 spread 前已有固定实参。
  它读取唯一的编译器 source-expression、真实 List 构造器和固定 callable 契约，按通用类型关系逐项证明，
  保留 head、实参的单次求值与顺序。预览先运行
  `calcit fix --rule spread-call-proof-v1 --ns app.main --def run --format edn`；review 后再使用现有
  `--apply --expect-revision <revision>`，写回仍经过 staged preprocess 和原子事务。该规则不进入默认 preset。
  未知长度、开放实参、optional/rest、尚未证明的 trait bounds、多重 spread 或不唯一的 macro/source 映射
  返回 `requires-review`，replacement 为空，传入 `--apply` 也不修改这些节点。quoted data 和函数参数声明不作为调用检查。
- `unsafe-coerce-boundary-v1` 把现有 `E_UNSCOPED_UNSAFE_COERCE` 转为可导航的待审建议，运行
  `calcit fix --rule unsafe-coerce-boundary-v1 --ns app.main --def run --format edn` 查看编译器位置、源码指纹、
  词法函数位置与编译栈。先核对捕获、效果和失败路径，再人工选择 checked decoder 或显式 adapter。
  建议的 replacement 为空，`--apply` 不修改源码或授予 `:js-ffi`。编译错误保留在 diagnostics 中，命令仍非零退出，
  不能用预览成功替代 strict 编译。已有明确词法权限的 adapter 不生成建议；词法权限以编译器规则为准，
  不因函数名、namespace 命名或调用者的权限推断独立 source definition 的权限。
  此规则须使用严格类型模式，不进入默认 preset 或 strict workflow 的自动改写集合；只检查 definition `:code`，
  每个 definition 在首个编译错误停止。修复后重新运行以发现后续错误；`:tests` / `:examples` 仍需独立验证。
  不相关错误、依赖或所选 scope 外的错误、缺少源码定位的错误直接失败，不猜定位或隐藏问题。
- `assert-type-proof-v1` 显式审计断言前的类型证据，运行
  `calcit fix --rule assert-type-proof-v1 --ns app.main --def run --format edn` 查看原始输入位置和编译栈。
  已知矛盾保留 `E_ASSERT_TYPE_MISMATCH`；开放输入、未知 callable 或未绑定泛型缺少证明时报告
  `E_ASSERT_TYPE_UNPROVEN`。审计在更新局部类型之前停止，因此不能通过再加一层断言消除待审边界。
  当前预处理涉及的函数体还须独立证明其具体返回契约；仅靠返回声明的 producer 报告
  `E_FN_RETURN_UNPROVEN`，并定位其真实 source owner。Number identity、泛型 T identity 和明确的
  Dynamic 保存/传递可以保持原写法；未知 T 不能靠目标 Number 获得证明。
  producer 实现与返回声明矛盾时，既有 `W_FN_RETURN_TYPE_MISMATCH` 同样使审计失败，不能借用该声明获得空建议。
  将报告中的 definition 与 path 传给 `calcit query type-at <definition> --path <path> --format edn`
  可读取同一源码节点；展开后无法保留尾表达式坐标时，path 为声明根节点 `code`。
  规则复用 compiler 的共同证明关系与既有 suggestion，不运行程序猜类型，也不自动补 schema、选择 decoder、
  删除断言或插入强转。replacement 为空；`--apply` 不写入源码。报告保留 error，命令非零退出。
  此项审计须使用严格模式，暂不进入默认 preset 或 strict workflow；普通编译的开放边界迁移策略保持不变。
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

`core-api-0.28-v1` 从已发布的 `0.28.0-alpha.2` 起提供，组合已证明等价的 15 条叶子改写规则。它适合在结构升级后一次迁移核心 API 名称；每个调用仍须满足对应规则的类型、来源和源码位置证明。表中的规则也可用 `--rule` 单独选择，未回填进旧的 `surface-latest-v1/v2`；不要把“旧 preset 不变”理解为需要逐条运行所有规则。

| 范围 | 展开的规则 |
| --- | --- |
| nil/整数谓词与标量转换 | `core-non-nil-predicate-v1`、`core-integer-predicate-v1`、`core-identity-conversion-v1` |
| 索引/键/值/成员判断 | `core-predicate-method-v1` |
| 集合追加、长度与折叠 | `core-list-add-v1`、`core-set-include-v1`、`core-collection-len-v1`、`core-list-fold-v1` |
| 分隔、展平映射、文本拼接与读取 | `core-list-intersperse-v1`、`core-list-flat-map-v1`、`core-list-join-string-v1`、`core-list-get-v1` |
| Map/Set 组合与效果方法 | `core-map-distinct-values-v1`、`core-collection-combine-v1`、`core-effect-method-v1` |

```bash
calcit calcit.cirru fix --preset core-api-0.28-v1 --format edn
calcit calcit.cirru fix --preset core-api-0.28-v1 \
  --apply --expect-revision 'md5:<preview 返回的 revision>'
calcit calcit.cirru fix --preset core-api-0.28-v1 --format edn
```

此集合只替换 definition `:code` 内已证明的叶子，嵌套调用的参数和 source path 保持原位。预览的 `:data :filters :source-coverage` 在 `:scanned-regions` 列出字符串 `|code`，在 `:manual-review-regions` 列出 `|tests`、`|examples`。未证明的候选继续作为 `requires-review` 返回，quoted data 和 macro 定义沿用各规则的边界。普通函数、primitive 和方法参数的求值上下文由 reader 或 compiler-resolved 来源确认，未知宏仍需人工核对其是否观察源码拼写。

应用后重复预览，`machine-applicable` 建议应为空；`requires-review` 候选可能仍在，不能把它们当成已迁移或为了清零而自动应用。逐项核对这些候选以及 attached `:tests` / `:examples`，再运行项目严格检查和测试。使用已发布 `0.28.0-alpha.2` 的 Respo 副本验证中自动迁移 15 处后没有剩余可自动应用的建议，32 处人工审阅候选保留，48 个 definition tests 全部通过；这不代表整个项目的旧名已经清零。

先用 `surface-latest-v2` 整理构造器和 `do` 结构，再运行本 preset；如需迁移 `%some/%none/%ok/%err` 或 `option:*/result:*` helper，分别使用 `core-nominal-constructor-v1`、`core-option-method-v1`、`core-result-method-v1` 并逐次核对新 revision。这些规则会改写完整调用树，不能和叶子集合未经组合证明就一次应用。旧 `surface-latest-v1/v2` 及 `--workflow strict` 的冻结范围保持原有定义。

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
`:applicability :machine-applicable` 且未留下 `Dynamic`/未知 callable 的候选会生成事务 operation。`needs-review`
候选只展示更精确的外层结构，例如 `Option<Dynamic>`，不会把未知 payload 扩大成整个 `Dynamic`，也不会在 `--apply`
时部分写回。无法从静态实现恢复类型时命令明确失败；macro、nominal data、trait 和 impl contract 必须继续显式声明。

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
