# 类型安全的 Option 查询终点 RFC

状态：Withdrawn（2026-08-26；六个 `*-or` 宏已从 core 移除）
日期：2026-08-23

> 历史说明：实际生态采用率为零，而这些宏只隐藏了一次明确的
> `option:unwrap-or` 调用。当前代码应直接调用查询函数并对返回的 `Option`
> 使用 `.unwrap-or`；下文保留为设计与撤回记录。

## 背景与生态证据

`08-05-systematic-nil-reduction-rfc.md` 把公开缺失值统一为 `Option<T>`，边界已经比旧的
`nil` 契约安全。但迁移后的业务代码出现了新的机械噪音：一次普通查询往往立即接
`option:unwrap-or`，把“查找”和“缺失时采用业务默认值”拆成两层调用。

对 2026-08-23 同步到各仓库默认分支后的本地 Calcit Snapshot 样本盘点发现：

- 约 102 个 Calcit 项目包含 unwrap 相关调用；
- `unwrap-or` 约 1500 次，直接 `unwrap` 约 300 次；
- 单行可识别的 `option:unwrap-or (get ...)` 约 985 次；
- `get-env`、`first`、`last`、`nth` 后立即提供默认值也是重复模式；
- 典型业务仓库包括 Respo 组件、Lilac、Termina 服务和 Cumulo 应用。

这些调用可分为四类：

1. 查询后立即采用默认值；
2. 根据 some/none 分支并绑定 payload；
3. 由前置条件证明必然存在的内部不变量；
4. Result 的可恢复错误处理。

第一类数量最大，适合由 core API 直接简化。第二类已经由 `if-let`、`when-let` 和
`match` 覆盖。第三类仍需显式 `.unwrap`，但应保持少量且靠近证明。第四类不能被
Option 默认值 API 吞掉错误信息。

## 设计原则

- 不引入 `Option<T> -> T` 隐式转换；
- 不让查询函数的原始返回类型随上下文变化；
- 默认值必须经过现有 `.unwrap-or` 泛型检查；payload 可推断时，类型不兼容继续产生诊断，Dynamic 边界不伪造额外证据；
- 需要区分“存在”和“缺失”时，继续返回并处理完整 `Option`；
- 已知 Struct 字段继续使用 `(:field value)`，不允许借查询 helper 绕过字段检查；
- 名称明确表达这是“以默认值结束查询”，而不是一般的错误恢复。

## API

新增以下 core 宏：

| API | 展开语义 | 适用边界 |
| --- | --- | --- |
| `get-or base key fallback` | `option:unwrap-or (get base key) fallback` | Map / List / String / Enum 查询 |
| `get-in-or base path fallback` | `option:unwrap-or (get-in base path) fallback` | 开放动态路径 |
| `get-env-or name fallback` | `option:unwrap-or (get-env name) fallback` | 环境变量 |
| `first-or xs fallback` | `option:unwrap-or (first xs) fallback` | 空集合默认值 |
| `last-or xs fallback` | `option:unwrap-or (last xs) fallback` | 空集合默认值 |
| `nth-or xs idx fallback` | `option:unwrap-or (nth xs idx) fallback` | 越界默认值 |

使用宏而不是给原查询函数增加多重返回类型：`get x k` 始终保持 `Option<T>`，不会因为
第三个参数或期望类型而改变契约。宏展开后仍由已有 Option helper 完成泛型绑定和参数检查；
若来源本来就是 `Option<Dynamic>`，结果也不会假装获得具体 payload 类型。

```cirru.no-check
let
    port $ get-or config :port 6000
    mode $ get-env-or |mode |release
    title $ get-in-or data ([] :page :title) |Untitled
  println port mode title
```

fallback 沿用普通函数参数的 eager 求值语义。惰性 fallback 暂不开放：验证中发现
`option:fold` 尚未强制两个回调共享返回类型，已记录为 issue #388。修复并验证该类型缺口后，
可以单独评估 `*-or-else`，不能在当前 API 中悄悄改变副作用顺序。

## 分支与不变量

查询值需要分支时不使用 `*-or`：

```cirru.no-check
if-let
  user $ get users user-id
  render-user user
  render-missing user-id
```

`if-let` 消费 `Option<T>` 并只在 some 分支绑定 `T`；`when-let` 用于返回
`Option<R>` 的单分支组合；完整数据流优先使用穷尽 `match`。

`.unwrap` 只保留在已有控制流、断言或数据结构不变量确实证明 some 的位置。迁移工具不得把
`.unwrap` 机械替换为任意空值，也不得用 `unsafe-coerce` 擦除 Option。

## 迁移与验收

首轮生态迁移优先处理机械可验证的形式：

```cirru.no-check
option:unwrap-or (get config :port) 6000
; =>
get-or config :port 6000
```

迁移后必须运行当前项目的 `--check-only`、测试和对应 JS/native 构建。验收至少包括：

1. 六个宏的存在与缺失路径测试；
2. fallback 类型不兼容的负向诊断；
3. Native 与 JS 的真实项目回归；
4. 文档示例检查；
5. 发布后在代表性生态仓库中证明辅助调用数量下降。

## 非目标

- 不改变 `get`、`get-in`、`first`、`last`、`nth`、`get-env` 的 Option 返回契约；
- 不为 Result 提供会丢弃错误信息的查询别名；
- 不删除 `option:unwrap-or` 的内部 lowering 实现；
- 不保证业务 fallback 是惰性表达式；
- 不自动修改 Dynamic 或旧 Optional/JsNullish 边界。
