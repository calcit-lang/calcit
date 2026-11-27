---
title: "错误处理与 Result 契约"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "error handling"
  - "result"
  - "try raise"
  - "exception handling"
  - "errors"
---

# 错误处理与 `Result` 契约

Calcit 将错误明确分为两层：**可恢复的业务/解析/IO 失败使用名义枚举 `Result<T, E>`（或 `Option<T>`）表达并在类型中传递，不可恢复的程序缺陷与契约违反使用 `raise` / `unreachable!` / `todo!` / `assert` 立即终止（在 WASM/WASI 中 trap）**。

在 WASI server 与 WASM 目标上，业务代码中的 `try` 异常处理已经退役，编译期会以 `E_WASM_UNSUPPORTED_TRY` 明确拒绝；`try` 仅保留给 native 与 JavaScript 目标上的宿主兼容边界或测试断言辅助（如 `calcit.test/throws?`）。

## 速览

- **返回可恢复失败**：`Result :ok value` / `Result :err error`
- **分支处理与链式组合**：`match`、`.map`、`.map-err`、`.and-then`、`.or-else`、`.unwrap-or`
- **自定义结构化错误**：定义领域错误 `defenum`，并作为 `Result T MyError` 的错误类型参数
- **不可恢复缺陷**：`raise |reason`、`unreachable! "|why impossible"`、`todo! "|pending work"`
- **WASI/WASM 边界**：不使用 `try` 恢复异常，统一传递 `Result<T, E>`；不可恢复失败直接 trap

## 可恢复错误：`Result<T, E>`

当失败属于正常控制流的一部分——例如用户输入格式错误、JSON / Cirru EDN 解析失败、数值范围超限、路径缺失、环境变量或文件读写失败——函数应返回 `Result<T, E>`（仅表示值缺失时使用 `Option<T>`），而不是依赖 `raise` + `try`。

```cirru
let
    safe-div $ fn (a b)
      hint-fn $ Fn Number Number $ -> (Result Number String)
      if (= b 0)
        Result :err |division-by-zero
        Result :ok (/ a b)
    render $ fn (res)
      hint-fn $ Fn (Result Number String) $ -> String
      match res
        (:ok value) (str "|ok: " value)
        (:err message) (str "|err: " message)
  assert= "|ok: 5" $ render (safe-div 10 2)
  assert= "|err: division-by-zero" $ render (safe-div 10 0)
```

`Result` 提供常用的值语义组合方法，可把解码、校验与默认值回退串联为线性数据流，在 native、JavaScript 与 WASM/WASI 下具有一致语义：

```cirru
let
    parse-positive-int $ fn (text)
      hint-fn $ Fn String $ -> (Result Number String)
      -> text
        .parse-float
        .and-then $ fn (value)
          if (and (> value 0) (= value (floor value)))
            Result :ok value
            Result :err |expected-positive-integer
        .map $ fn (n) (* n 10)
        .map-err $ fn (err) (str "|invalid: " err)
  assert= (Result :ok 40) (parse-positive-int |4)
  assert= true $ .err? (parse-positive-int |-2)
  assert= 0 $ .unwrap-or (parse-positive-int |bad) 0
```

需要区分错误类别以便上层分流（如 HTTP 状态码映射或重试策略）时，应定义名义错误枚举而不是把一切错误编码成非结构化字符串：

```cirru
let
    ParseError $ defenum ParseError
      :empty
      :negative Number
      :syntax String
    parse-port $ fn (raw)
      hint-fn $ Fn String $ -> (Result Number ParseError)
      if (blank? raw)
        Result :err (ParseError :empty)
        match (.parse-float raw)
          (:err msg) (Result :err (ParseError :syntax msg))
          (:ok n)
            if (<= n 0)
              Result :err (ParseError :negative n)
              Result :ok n
  assert= (Result :ok 8080) (parse-port |8080)
  assert= (Result :err (ParseError :empty)) (parse-port "|  ")
```

## 标准库中的可恢复边界

core 中与服务端和边界解码相关的 API 已直接提供 `Result` / `Option` 契约，不隐藏内部 `try`：

| 场景 | 推荐入口 | 返回契约 |
| --- | --- | --- |
| 浮点数解析 | `parse-float` / `String .parse-float` | `Result Number String` |
| JSON 文本解析 | `try-parse-json` / `String .parse-json` | `Result Dynamic String`（随后配合 `try-decode-map-as` 收窄） |
| 闭合 Cirru EDN 解码 | `try-parse-cirru-edn-as text Type` | `Result T String` |
| 开放值闭合解码 | `try-decode-map-as value Type` | `Result T String` |
| Cirru 源码 / EDN 列表（native / JS） | `try-parse-cirru`、`.parse-cirru-edn`、`.parse-cirru-list` | `Result<..., String>` |
| 深层容器路径读取 | `get-in value path` | `Result Dynamic PathError` |
| 集合索引 / 环境变量 | `get`、`first`、`last`、`nth`、`get-env` | `Option T` |
| 文件与目录读取 | `FsPath .read-text`、`.read-dir`、`.write-text!`、`read-stdin-text` | `Result<..., String>` |

闭合解码的目标可以是 `deftype` 别名，包括已填入类型参数的泛型别名，或绑定到此类类型的类型槽。解码仍按展开后的类型检查实际值；未绑定泛型和递归别名不会成为开放的 `Dynamic`。`try-decode-map-as` 的运行时解码支持 native 和 JS，WASM 暂不支持。

## 不可恢复失败：`raise`、`unreachable!`、`todo!` 与 `assert`

只有在违反前置条件、出现逻辑不变量破坏或进入不可达分支时，才使用 `raise` 或控制流标记终止执行（效果等同于 Rust 的 `panic!`）：

- **`raise value`**：终止当前执行并报告错误；native 抛出 effect 错误，JavaScript 抛出 `Error`，WASM/WASI 触发运行时 trap。`case` 缺少匹配分支、`non-nil!` 遇到 `nil` 也属于不可恢复失败。
- **`unreachable! [message]`**：标记静态实现上逻辑不可达的分支（如 `match` 兜底分支）。类型为 `Never`，不污染 `match` 分支合并推断；传入非 `String` 报 `E_PROC_ARG_TYPE_MISMATCH`。运行时到达则报告所在定义与消息，在 WASM/WASI 中 trap。
- **`todo! [message]`**：标记尚未完成的实现脚手架，接收零或一个静态字符串。静态预处理会报 `E_TODO` 提示补全，避免误把“未完成”当作业务 `raise`；运行时到达则在 native/JS 抛出 TODO 错误，在 WASM/WASI 中 trap。
- **`assert` / `assert=`**：检查必须为 `Bool` 的条件（#2881 起 `assert` 条件非 `Bool` 静态报 `E_NON_BOOL_CONDITION`）或两值相等，失败时中止运行，并在测试门禁中作为断言失败暴露。

```cirru.no-check
defn validate-order (order)
  todo! "|implement order validation"
```

```cirru.no-check
defn shape-area (shape)
  match shape
    (:circle r) (* 3.14 r r)
    (:square w) (* w w)
    _ $ unreachable! "|Shape only has :circle and :square"
```

每个内建 Proc 的失败类别（`total`、`result` 或 `raises` 及其条件）可通过 `calcit query def` 查看，详见[查询命令](../run/query.md)。

## `try` 在 WASI/WASM 的退役边界与兼容规则

- **WASI server / WASM 退役**：`try` 在 `calcit wasm` 与 `calcit wasi` 中已退役，编译可达的 `try` 表达式会立即失败并报告 `E_WASM_UNSUPPORTED_TRY`，提示将可恢复分支改写为 `Result :ok` / `Result :err` 与 `match`，并将真正不可恢复的不变量失败保留给 `raise` trap。
- **核心库零隐藏 `try`**：`calcit.core` 自身已移除 `try`（包括 `try-parse-*` 与 `try-read-dir` / `.walk-dir`）。这消除了这些函数对异常捕获的隐式依赖；动态 Cirru 解析和递归目录读取仍受 WASM/WASI 能力限制。WASI 的 `try-read-dir` 支持非递归读取，递归请求返回含 `E_WASM_CAPABILITY` 的 `Result :err`；直接使用 `.walk-dir` 在编译时报告此限制。`calcit.test/throws?` 与 `is-throws` 保留为 native/JS 的断言异常捕获辅助。
- **native / JS 宿主兼容边界**：在 native 与生成 JavaScript 中，`try body handler` 仍可作为调用遗留会 `raise` 的第三方/宿主 API 时的最外层转换边界，把捕获的错误归一化为 `String` 后立刻转为 `Result :err`：

```cirru
let
    safe-wrap $ fn (a b)
      hint-fn $ Fn Number Number $ -> (Result Number String)
      try
        Result :ok
          if (= b 0) (raise |division-by-zero) (/ a b)
        fn (message) (Result :err message)
  assert= (Result :ok 5) (safe-wrap 10 2)
  assert= (Result :err |division-by-zero) (safe-wrap 10 0)
```

### `try` 的类型推断（仅 native / JS）

`try` 合并正常表达式与错误处理器的返回类型。错误处理器接收归一化后的 `String`；未标注的匿名函数可从此处获得参数类型，返回类型由函数体独立推断。正常分支直接 `raise` 时只采用处理器类型；处理器直接 `raise` 时只保留正常分支类型。`Option` 与 `Result` 构造分支使用名义分支合流规则。

```cirru
let
    result $ try (Result :ok 7)
      fn (message) (Result :err message)
  assert-type result $ Result Number String
```

## 限制

- WASI 与 WASM 目标不支持任何形式的可恢复异常捕获（`E_WASM_UNSUPPORTED_TRY`），调用旧测试断言 `throws?` / `is-throws` 或含 `try` 的定义仅限 native 与 JavaScript。
- 变参 `try` handler 仅可通过自身的 `hint-fn :rest 'String` 描述错误列表，`List<String>` 与固定参数尾部省略 `Option` 的规则也仅作用于 native 与 JavaScript。
- `try` 的 `Dynamic` 返回值仍需在消费者边界通过 `try-decode-map-as` 或类型谓词提供证明，且不会向处理器授予 `:js-ffi` 权限。
- 已有业务代码中借助 `raise` + `try` 表达的异常流不会自动提供 `calcit fix` 一键改写规则，需结合接口返回契约人工重构为 `Result<T, E>`。

## 相关文档

- [枚举与 Result/Option](enums.md)
- [WASM 支持矩阵](../installation/wasm-support.md)
- [静态分析与类型证明](static-analysis.md)
- [项目升级手册](../run/upgrade.md)
