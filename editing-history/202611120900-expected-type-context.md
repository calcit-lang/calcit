# 期望类型上下文：lambda 与裸 tag 共用一套位置规则 (#2733)

- 决定：`tail_context` 装的内容从 `Fn` 签名放宽成任意期望类型（`Expected { annotation, position }`），成为检查器里「这个位置期望什么类型」的唯一通道。来源是 2026-11-12 的硬编码审计：无标注 lambda 与裸 tag 能否通过取决于所在位置，每次修复都是再补一个位置。维护者确认双向检查要大力加强。
- 提供方：函数声明的返回类型（原来只在返回类型是 `Fn` 时提供）、`preprocess_argument_with_context` 的期望参数类型（新增）。`if` 分支、`&let` 末项、宏展开原本就转发，不用改。
- 使用方：lambda 取 `Fn` 签名（原有）；裸 tag 在 `preprocess_expr` 入口按期望 enum 改写（新增）；`[]` / `#{}` / `&{}` 字面量在入口把成员类型交给元素（新增，复用参数位置已有的字面量逻辑，元素里的裸 tag 在 `preprocess_literal_member` 改写）。
- 保守处：含类型变量的期望类型不向下提供，也不用于 tag 和字面量，避免把被调函数的泛型变量当成调用方的类型。直接作参数的裸 tag 仍由调用处改写，诊断里的位置文字（`arg N of f`）不变。
- 未做：enum 构造的 payload（`(Some :red)`）、泛型参数绑定后的 tag（`(= c :red)`）、`match` 分支转发；`coerce_tail_tag_literals` 因 `match` 尾分支仍保留。`let` 绑定的裸 tag 不从之后的使用处反推，保持报错。
- 验证：`cargo test --test expected_type_context_cli --test enum_tag_coercion_cli`，fixture `tests/fixtures/expected-type-context.cirru`。
