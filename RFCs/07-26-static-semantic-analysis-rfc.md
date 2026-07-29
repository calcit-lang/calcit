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

验收包括：稳定 diagnostic code/phase、expected/actual 为结构数据、`type-at` 不执行程序、故意 FFI dynamic 不被误报，以及正常/未知/显式 schema 分支的回归测试。
