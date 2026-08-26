# Option / Result 顺序绑定宏 RFC

状态：Partially retained（2026-08-26；保留 `option:let`，移除 `result:let`）
日期：2026-08-23

> 历史说明：`option:let` 在 Respo 中有实际使用，因此继续保留；`result:let`
> 没有生态采用且会隐藏 Result 链，已从 core 移除。Result 代码应直接使用
> receiver-first `.and-then`。下文保留原始实验设计供追溯。

## 目标

在不增加 parser syntax、提前返回控制流或隐式 unwrap 的前提下，简化连续的
`Option` / `Result` 计算。实验能力完全由 core macro 提供，并展开为已有的
`.and-then` 方法调用。

本 RFC 不继续推进 labelled arguments。label 会同时进入参数声明、函数类型、
调用绑定、偏函数与各 codegen 后端，现阶段复杂度高于收益。

## API

```cirru.no-check
option:let
    user $ get users user-id
    profile $ get user :profile
  %some $ render profile

result:let
    content $ read-file path
    data $ parse-data content
  save-data data
```

两个宏沿用现有 `let` 的 binding pair 结构。每个右侧表达式必须返回对应容器，
body 也必须显式返回对应容器；宏不会自动添加 `%some` / `%ok`。

`result:let` 展开为：

```cirru.no-check
.and-then (read-file path) $ fn (content)
  .and-then (parse-data content) $ fn (data)
    save-data data
```

`option:let` 同样展开为嵌套的 `.and-then`。因此每个表达式只求值一次，
`:none` / `:err` 由现有方法原样短路传播，错误类型不同仍需显式 `.map-err`。

## 方法优先的公开边界

普通函数能力优先通过 Option / Result 的 method bag 暴露。用户代码应写
`.map`、`.and-then`、`.or-else`、`.map-err` 和 `.unwrap-or`，命名空间函数
`option:*` / `result:*` 保留为 core lowering 与动态边界。

`option:let` / `result:let` 自身是控制代码展开的宏，不能通过运行时 method
dispatch 调用，因此保留命名空间形式；它们生成的组合代码仍使用 `.and-then`。
后续若增加 `fold`、lazy fallback、`flatten` 等普通函数，必须同步注册到对应
method bag，并以接收者方法作为文档主用法。

## 诊断与边界

- bindings 必须是由二元 pair 组成的 list；左侧必须是 symbol；
- body 至少包含一个表达式；
- 不提供 Rust `?` 式外围函数 early return；macro 只控制自己包裹的 continuation；
- 不在 Option 与 Result 之间隐式转换；
- 不接受 nil / JsNullish 作为容器替代；
- macro schema 本身保持 Dynamic AST 边界，展开结果继续接受普通方法 schema 检查。

## 验收

1. 多个 `:some` / `:ok` 绑定按顺序进入 body；
2. `:none` / `:err` 在后续表达式执行前短路；
3. body 返回裸值、容器混用和 Result 错误类型不一致时产生类型诊断；
4. macro expansion 只包含现有 `fn`、`do` 和 `.and-then` 调用；
5. Native 与 JS 构建和现有 core 测试全部通过。

实验阶段先在真实 Calcit 项目替换少量嵌套 `.and-then`。只有在类型诊断、错误位置
和生成代码都稳定后再移除 `:experimental`，不为缩短字符数增加新 parser syntax。
