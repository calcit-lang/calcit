---
title: "Effect Boundaries (:state / :io)"
summary: "函数用 :features :state / :io 声明读写全局 Ref、打印、文件、环境变量与时钟等效果，调用方随之声明；dbg 不计效果，由 entry 的 :dbg 策略控制"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "effects"
  - ":state"
  - ":io"
  - "dbg"
  - "feature-policy"
id: core/features/effects
parent: core/features
---

# 效果边界：`:state` / `:io`

函数在结构化 `Fn` schema 里用 `:features` 声明自己的效果。读写不是自己创建的 Ref 要声明 `:state`，打印、文件、环境变量、时钟、等待、随机字节和 `quit!` 要声明 `:io`：

```cirru.no-check
:schema $ quote $ Fn (-> Unit) (:features :state :io)
```

已有 `Fn` schema 时用命令补上，不必重写签名：

```bash
calcit edit schema 'app.main/render!' --add-feature state
calcit edit schema 'app.main/render!' --add-feature io
```

## 什么算使用

函数体里“出现即使用”。下面任一项出现在函数体中（包括没有自己 `hint-fn` 的 lambda），函数就需要对应的 feature：

1. 原始操作。`:state`：`deref`、`reset!`、`swap!`、`add-watch!`、`remove-watch!`、`generate-id!`。`:io`：`println`、`eprintln`、`echo`、`&display-stack`、`get-env`、`get-args`、文件与目录读写、`read-stdin-text`、`unix-time-ms`、`monotonic-time-ms`、`wait-ms`、`secure-random-bytes`、`quit!`。
2. 调用一个 schema 声明了 `:state` / `:io` 的定义。
3. 把这样的定义当作值传出去，例如 `.map xs log!`。
4. 调用一个类型带 feature 的 `Fn` 参数或局部变量。

因此 `.map`、`foldl`、`&doseq` 等高阶函数不需要改：lambda 的效果算在外层函数上，具名函数在引用处就算。

不算效果的情况：

- 对本函数里用 `ref` 创建的局部 Ref 读写，例如累加器。
- 以 `raise` 结尾的代码块里的 `eprintln`，即失败报告。`assert=` 等断言宏展开后就是这个形状。
- `dbg`，见下文。

自己带 `hint-fn` 且声明了 `:state` / `:io` 的 lambda 按自己的声明检查，创建它不算外层函数的使用，调用它才算。这样一个不打印的渲染函数里可以放一个打印的事件处理函数：

```cirru.no-check
defcomp comp-button ()
  button $ {}
    :on-click $ fn (e d!)
      hint-fn $ Fn Dynamic DispatchFn (-> Unit) (:features :io)
      println |clicked
```

## `Fn` 类型里的 features

从 0.43.0 起，`Fn` 类型兼容判断比较 `:state` / `:io`：带 `:io` 的函数只能放进同样声明了 `:io` 的 `Fn` 位置，参数、struct 字段、返回值等所有 `Fn` 位置都一样。声明更多效果的位置接受效果更少的函数。

```cirru.no-check
defstruct Handler (:on-click $ Fn Number (-> Unit))

; log! 声明了 :io，放进不带 :io 的字段会报 E_FN_ARG_TYPE_MISMATCH：
; expects type `fn(:number) -> :unit`, but got `fn(:number) -> :unit (:features :io)`
Handler :on-click log!

; 字段声明 :io 后可以存放
defstruct Handler (:on-click $ Fn Number (-> Unit) (:features :io))
```

`map`、`each`、`foldl`、`filter`、`find`、`sort`、`update` 等在返回前就调用回调的 core 高阶函数，回调位置标了 `(:features :pass)`。这样的位置接受任意效果的回调，效果算在传入回调的调用方上：

```cirru.no-check
:schema $ quote $ Fn (List T)
  Fn T (-> U) (:features :pass)
  -> $ List U
  :generics T U
```

- 具名函数在引用处就算（见上文第 3 条）。
- 自带 `hint-fn` 并声明了效果的 lambda，或类型带 feature 的局部 `Fn`，传进 `:pass` 位置时也算外层函数的使用。
- 没有自己 `hint-fn` 的 lambda 不带 `:state` / `:io`，它的效果本来就算在外层函数上，因此可以放进任何 `Fn` 位置。

项目自己的高阶函数如果也是在返回前调用回调，可以在回调参数上写 `(:features :pass)`。存起来以后再调用的回调（例如 `add-watch!` 的 watcher、事件处理函数）不要用 `:pass`，应在位置上写出它允许的效果。

`:js-ffi` 仍是词法规则：只约束函数自己的函数体，调用 FFI 包装函数不需要声明。`:state` / `:io` 描述调用方能观察到的行为，所以会向调用方传递。

## entry 策略

entry 的 `:feature-policy` 控制检查强度，取值 `:allow`、`:warn`、`:error`：

```cirru.no-check
:feature-policy $ {} (:state :error) (:io :error) (:dbg :error)
```

- entry 未写 `:state` / `:io` 时按 `:error`：报 `E_STATE_FEATURE_REQUIRED` / `E_IO_FEATURE_REQUIRED`，检查失败。
- 设为 `:warn` 时报 `W_STATE_FEATURE_REQUIRED` / `W_IO_FEATURE_REQUIRED`，检查仍然通过；设为 `:allow` 时不检查。
- 同一函数的同一 feature 只报第一处。

```bash
calcit config set feature-policy.state error
calcit config set feature-policy.io error
```

## 批量补声明

`--check-only` 只报告当前缺声明的函数；给它们补上 feature 后，调用它们的函数才会在下一轮报出来。`calcit fix --rule effect-features-v1` 从编译结果一次算到不动点，给所有带 `Fn` schema 的项目函数补齐缺少的 `:state` / `:io`：

```bash
calcit fix --rule effect-features-v1 --format json
calcit fix --rule effect-features-v1 --apply --expect-revision <revision>
```

每条建议的 `origin_chain` 说明原因：`direct-effect` 是函数体自己的操作，`dependency` 是用到的另一个定义。`--ns` / `--def` 只筛选输出的建议，传递关系仍按整个项目计算。规则只添加 feature，不删除已有声明，也不修改 lambda 自己的 `hint-fn`。

## `dbg`

`dbg` 打印表达式的源码和值，原样返回值，不算 `:io`，使用它的函数不需要声明：

```cirru.no-check
let
    total $ dbg $ foldl xs 0 &+
  * total 2
```

发布用的 entry 可以禁止 `dbg`：`:feature-policy` 的 `:dbg` 缺省 `:allow`，`:warn` 报 `W_DBG_IN_ENTRY`，`:error` 报 `E_DBG_IN_ENTRY`。`println` 始终算 `:io`。

## 限制

- 只检查有结构化 `Fn` schema 的定义；没有 schema 的定义（包括 `calcit eval` 片段）不检查。
- `Fn` 类型兼容只比较 `:state` / `:io`（以及 `:pass` 位置），`:js-ffi` 等其他 feature 不参与。
- 类型为 `Dynamic` 或裸 `Fn` 的位置接受任何函数，存进去的效果不再被追踪。
- 类型带 `:pass` 的回调参数在函数体里不带具体效果；把它再存进别的 `Fn` 位置时，效果不再被追踪。
- 通过局部别名（`let run map`）调用高阶函数时，传进 `:pass` 位置的 lambda 不会计到外层函数上。
- 方法调用（`.deref` 等）不参与判定。
- 只检查项目自己的命名空间，依赖模块的诊断不出现在应用的检查结果里。
