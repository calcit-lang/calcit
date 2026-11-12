# 期望类型上下文：lambda 与裸 tag 共用一套位置规则 (#2733)

- 决定：`tail_context` 装的内容从 `Fn` 签名放宽成任意期望类型（`Expected { annotation, position }`），成为检查器里「这个位置期望什么类型」的唯一通道。来源是 2026-11-12 的硬编码审计：无标注 lambda 与裸 tag 能否通过取决于所在位置，每次修复都是再补一个位置。维护者确认双向检查要大力加强。
- 提供方：函数声明的返回类型（原来只在返回类型是 `Fn` 时提供）、`preprocess_argument_with_context` 的期望参数类型（新增）。`if` 分支、`&let` 末项、宏展开原本就转发，不用改。
- 使用方：lambda 取 `Fn` 签名（原有）；裸 tag 在 `preprocess_expr` 入口按期望 enum 改写（新增）；`[]` / `#{}` / `&{}` 字面量在入口把成员类型交给元素（新增，复用参数位置已有的字面量逻辑，元素里的裸 tag 在 `preprocess_literal_member` 改写）。
- 保守处：含类型变量的期望类型不向下提供，也不用于 tag 和字面量，避免把被调函数的泛型变量当成调用方的类型。直接作参数的裸 tag 仍由调用处改写，诊断里的位置文字（`arg N of f`）不变。
- 未做：enum 构造的 payload（`(Some :red)`）、泛型参数绑定后的 tag（`(= c :red)`）、`match` 分支转发；`coerce_tail_tag_literals` 因 `match` 尾分支仍保留。`let` 绑定的裸 tag 不从之后的使用处反推，保持报错。
- 验证：`cargo test --test expected_type_context_cli --test enum_tag_coercion_cli`，fixture `tests/fixtures/expected-type-context.cirru`。

## 第二步：`match` 分支、variant payload、`=` 的后续操作数

- `match` 在处理被匹配的值之前取走自己位置上的期望类型，再交给每个分支体（`MacroSourcePhase::process_expected`）。尾位置的 `match` 分支因此与其他位置走同一条通道，`coerce_tail_tag_literals` 这个逐位置补丁删除。
- `Expected` 增加 `location`：裸 tag 自身没有源码坐标，函数返回位置提供调用或定义的位置，保持原来 `match` 尾分支诊断的定位。
- enum 构造的 payload：构造调用本来就按分支声明算出 payload 的期望类型，现在裸 tag 在这里按它改写（位置文字 `variant payload`）。`Some x` 写在期望 `Option T` 的位置时，入口从期望类型取 `T` 作为 payload 类型，与实参位置已有的 D4 规则一致；期望类型留在原处，供 `if` 等继续转发。
- 实参位置的 tag：期望类型已经用前面实参绑定的类型变量替换过，所以不再要求原始声明不含类型变量；变长参数（`Variadic T`）取元素类型，超出声明个数的参数用替换后的 rest 类型。`(= c :red)` 因此按 `c` 的 enum 读 tag。
- 修正第一步的回归：按成员类型下传的字面量绕过了构造调用上的隐式 nil 分支检查，入口转发后对成员补同一检查（`nil_branch_contracts_keep_every_main_rejection`）。
- 仍未做：`EXPECTED_FN_TYPE` / `EXPECTED_STRUCT_TYPE` / `EXPECTED_OPTION_PAYLOAD` 三个线程局部量并入同一上下文。
- review 后补充：`match` 降级成 `&let` / `if` 链的三条路径（字面量模式、data view、union 类型模式）用 `TailScope::forward` 把期望类型交给降级后的表达式；`Some` 的 payload 上下文只在构造表达式自身（head 是 `Some` / `Option`）打开，否则会被条件或被匹配值里的第一个调用取走。

## 第三步：补提供方

- 依据：#2740 合并后用探针（`probes-2026-11-12/h2`）找还收不到期望类型的位置：第一轮 30 个里通过 14 个，这一步之后 37 个里通过 22 个，其余按原因分组。
- 方法实参与局部函数实参：通用调用路径本来就为这些位置算出期望类型（给 lambda 用），裸 tag 现在也按它改写，位置文字是 `method argument` / `argument of a local function`。方法的类型变量只由接收者和前面的实参确定，tag 自身不算证据，所以重新用「接收者加前面的实参」算一次该位置的类型。
- 接收者确定的 core 参数：`assoc` 在 struct 上把值的类型定为字段类型（#2722 已用于直接 tag），现在字面量实参在预处理之前就取这个类型，成员按它检查。
- Tuple：`[]` 字面量项数与 Tuple 位置数相同时，每一项取对应位置的类型。项数不同不改写，仍由原来的检查报错。
- 未做（第三步时）：`recur` 实参（检查在函数体处理完之后才遍历，改写需要把外层函数的参数类型带进调用路径）；`assert-type` 是否作为提供方（它是证明义务，不是转换，需要先定语义）；`append` / `.append` / `contains?` 等签名偏弱的 proc（归 #2735）；用外层期望类型绑定被调函数的类型变量（`map xs (fn (x) :red)`）。
- review 后补充：字面量成员的隐式 nil 分支检查移到 `preprocess_argument_with_context` 的三个字面量分支里（Map、Tuple、List/Set），实参位置和入口转发共用。实参位置的 `List` 字面量原先也缺这项检查（`take-list ([] (if flag 1) 2)` 不报错），现在与普通 `[]` 调用一致，回归用例 `list-arg` / `tuple-arg`。

## 第四步：调用的期望类型反推被调函数的类型变量

- 问题：`map xs (fn (x) :red)` 写在期望 `List Color` 的位置时，回调的契约是 `Fn Number (-> MapOutput)`，结果变量要等回调体处理完才知道，回调体里的裸 tag 因此收不到期望类型。前三步都规定「含类型变量的期望类型不向里传」，这一类位置全部落空。
- 做法：调用处取自己位置上的期望类型（`offered_call_result`），与被调函数的结果类型做一次匹配（`offered_result_bindings`，复用 `prove_with_bindings`），得到结果变量的读数。读数只用于两处：回调参数的结果类型里仍有变量时，把代入后的类型作为回调返回位置的期望类型（`ExpectedFunctionContext.offered_return`，位置文字 `callback result`）；泛型的值参数代入后作为裸 tag 和字面量成员的期望类型（`offered_value_slot`）。
- 边界：读数不写入 `preprocessing_type_bindings`，也不改回调契约的返回类型，类型变量仍由实参自己的类型证明。回调返回其他类型时，报错仍是外层的返回类型不符，不新增诊断编号。期望类型含 `Dynamic` 或类型变量时不读。回调带 `hint-fn` 时按它自己声明的类型。
- 覆盖：具名函数调用（含 `map` 这类带 checked contract 的 core 函数，结果模式取 contract 的 `return_type`）、方法调用（`MethodArgumentContext.return_type`）、局部函数值。
- 依据：探针 `probes-2026-11-12/h3`，19 个正例由 0 个通过变为 17 个，8 个反例仍被拒绝。
- 未做：`foldl xs ([]) (fn (acc x) (conj acc :red))`（fold 系 proc 的签名是 Dynamic，归 #2735）；没有标注的 `let` 绑定没有期望类型，不反推。
- #2742 合并后的 review 意见一并处理：字面量成员的隐式 nil 分支检查收进 `preprocess_literal_member`（它知道成员所在槽位的类型），槽位声明为可空（`JsNullish T`）时不检查，与具名函数实参的规则一致；回归用例 `list-arg-nullable`。

## 第五步：`recur` 的实参

- 问题：`recur` 的实参检查在函数体处理完之后才遍历（`check_recur_args_in_expr`），预处理实参时不知道参数类型，所以 `recur (dec n) :green`、`recur (dec n) (fn (x) x)`、`recur (dec n) ([] :red)` 都按没有上下文处理，再被事后检查报 `E_RECUR_ARG_TYPE_MISMATCH`。
- 做法：所在函数的参数类型是词法事实，记在 `ScopeTypes.recur_parameters`（进入函数体时设置，嵌套函数各自覆盖）。通用调用路径里 head 是 `recur` 时，该位置的期望类型取对应参数的类型，之后与方法实参、局部函数实参走同一段代码：lambda、字面量、`if` 分支取上下文，裸 tag 改写（位置文字 `recur argument`）。事后的类型检查保留，类型不符仍报 `E_RECUR_ARG_TYPE_MISMATCH`。
- 边界：参数类型是 `Dynamic`、含类型变量或还是推断中的空槽位时不提供；有可选或 rest 参数的函数、宏不提供（与事后检查跳过的范围一致）。
- 依据：探针 `probes-2026-11-12/h4`，6 个原先失败的写法全部通过，`loop`、`Tag` 参数、直接自调用不受影响。

