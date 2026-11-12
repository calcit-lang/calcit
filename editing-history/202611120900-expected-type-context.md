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
