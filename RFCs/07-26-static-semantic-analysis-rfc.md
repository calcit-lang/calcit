# RFC: 静态语义发现、类型证据与结构化诊断

状态：Draft
日期：2026-07-26
关联：`03-05-function-schema-dual-track-rfc.md`、`07-26-agent-machine-protocol-rfc.md`

## 1. 目标

让人和 Agent 都能在不运行项目入口的条件下，获得 definition 或子树的可信静态信息：类型、method、trait、字段/variant、依赖证据与诊断。重点是置信度与边界清晰，不是把动态语言伪装成全静态语言。

## 2. 静态查询

```bash
cr query type :number --format json
cr query type ':: :list :number' --format json
cr query type app.schema/Person --format json
cr query type-at app.main/f --path code@3.2 --format json
```

返回 canonical type、可用 methods 与其 impl 优先级、schema/推断证据，以及可用时的 fields、enum variants、constructors、trait 和 examples。`type-at` 返回 inferred/expected type、lexical bindings、method candidates、evidence、definition revision 和 diagnostics。

处理只允许加载与预处理静态 metadata，不执行 init function。必须执行才能知道的信息明确返回 unknown/diagnostic，不得静默回退到 runtime。

## 3. 置信度与动态边界

工具必须区分：

- `proven`：schema 或静态规则已证明；
- `partial`：信息只覆盖一部分结构；
- `intentional-js-ffi` / `intentional-macro`：设计上允许的动态边界；
- `unresolved`：应补 schema 或推断规则；
- `unknown`：当前没有足够静态证据；
- `failed`：静态处理本身失败。

`:dynamic` 是“允许任意 Calcit 值”及“未知或放弃静态约束”的唯一语义。`:any` 是旧版本遗留的同义写法；解析时可兼容识别，但 schema、生成代码、文档与诊断输出都应迁移为 `:dynamic`。`check-types` 与 `weak-types` 不得把未识别 definition 或历史 `:any` 计为 full。

同质 collection/ref、命名 struct/enum、函数参数/返回/泛型/where/rest 以及 callback variance 应尽可能保留类型信息；真正的 global state、JS FFI、宏边界和异构值可保持明确的 dynamic。

### 3.1 多态能力的分层

“暂时不知道类型”不是多态。多态必须保存调用位置之间可以验证的关系，不能用多个互不相关的 `:dynamic` 代替：

| 需求 | 类型表达 | 静态收益 |
|---|---|---|
| 参数与返回值保持同一类型 | `:generics $ [] 'T`，在 `:args`/`:return` 复用 `'T` | 调用点绑定和返回类型替换 |
| 只要求值具有某些能力 | `'T` 加 trait `:where` bounds | method 候选校验与静态 specialization |
| 容器保持同质元素关系 | `:: :list 'T`、`:: :map 'K 'V`、`:: :ref 'T` | element/get/callback 类型继续传播 |
| 数据结构携带类型参数 | generic `defstruct` / `defenum` 的 applied type args | 构造、字段、variant payload 与 match 保持一致 |
| 回调的参数/返回关系 | 完整 `:: :fn`，保留 generics、rest 与 variance | 高阶函数调用检查，不把 callback 降为 `:fn`/`:dynamic` |
| 有限异构分支 | 命名 `defenum`，可空值用 `:: :optional T` | 穷举 variant/payload 检查 |
| 库声明、应用选择一个全局类型 | `deftype-slot` / `bind-type` | 跨编译单元注入；不是 per-call parametric polymorphism |

当前不引入通用 union/intersection 类型。已知的有限分支优先用 enum，nil 分支用 optional；只有开放世界输入、无法建模的 FFI/global state 或宏边界使用 `:dynamic`。Bare `:list`、`:map`、`:ref` 及 `:fn` 只保留外形，不能表达元素、状态或 callback 之间的多态关系，因此 coverage 最多为 partial。

泛型变量必须显式列在 `:generics`，`where` 只能约束已声明变量；applied struct/enum type args 必须满足 arity 与 where bounds。无法绑定的 TypeVar、dynamic callback slot 或 dynamic receiver 不得伪装成成功 specialization：分析结果必须保留 unresolved evidence，运行时兼容路径可以继续，但 Agent 应看到静态收益已经丢失。

### 3.2 Dynamic debt 的 Agent 引导

分析日志只在显式静态分析和 opt-in method warning 中出现，避免普通运行持续刷屏：

- `analyze check-types` 有 partial/none 时产生 `W_TYPE_COVERAGE_GAPS`，并提示继续运行 scoped `weak-types`；
- `analyze weak-types` 对 unresolved dynamic 产生 `W_DYNAMIC_TYPE_DEBT`，每个 occurrence 返回 `impact` 与 `suggestion`；
- `--warn-dyn-method` 保留精确调用点 warning，用于发现 dynamic receiver 阻止 trait/core method specialization 的位置；
- `intentional-js-ffi` 仍可见但不算 unresolved，提示在进入 typed code 前 validate/convert；
- `:any` 输入按 dynamic debt 处理，输出和修复建议统一写 `:dynamic`。

建议必须按关系给出修复方向，而不是机械地要求“给 dynamic 随便换一个类型”：同一类型关系用 TypeVar，能力约束用 trait/where，同质容器保留 type arg，有限异构值建模为 enum，真正边界才保留 dynamic。

## 4. CalcitDiagnostic

所有 parse、snapshot、macroexpand、preprocess、type-check、codegen、runtime 错误与 warning 逐步收敛到同一结构：

```json
{
  "code": "E_METHOD_NOT_FOUND",
  "phase": "preprocess",
  "severity": "error",
  "message": "...",
  "location": {
    "definition": "app.main/f",
    "path": "@3.2",
    "selector": "path ...",
    "fingerprint": "..."
  },
  "expected": [],
  "actual": null,
  "related": [],
  "fixes": []
}
```

`code` 稳定，`message` 可改善；location 的 path 是临时坐标，selector/fingerprint 才是重定位依据。`.calcit/error.cirru` 与 JSON CLI 从同一诊断模型渲染。fix 只能描述可预览的结构化 edit，不能静默写入。

## 5. 命令与验收

逐步提供：

```bash
cr --check-only --format json
cr query diagnostics --format json
cr analyze check-types --format json
cr analyze weak-types --intent unresolved --format json
```

验收包括：稳定 diagnostic code/phase、expected/actual 为结构数据、`type-at` 不执行程序、故意 FFI dynamic 不被误报、`:any` 输入 canonicalize 为 `:dynamic`、泛型/where/callback/container 中的 dynamic 能说明丢失的多态关系，以及正常/未知/显式 schema 分支的回归测试。
