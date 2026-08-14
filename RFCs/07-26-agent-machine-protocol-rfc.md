# RFC: Agent 机器语义协议与按需解析

状态：Draft
日期：2026-07-26
关联：`07-06-semantic-tree-navigation-rfc.md`、`07-26-static-semantic-analysis-rfc.md`

## 1. 概要

Calcit 的源码、定义元数据与编辑对象本来就是 Cirru EDN 树。Agent 接口应直接暴露这层语义，而不是退回到行号、正则或文本 patch。已有 `query context`、`query type`、`query type-at`、`analyze check-types/weak-types` 是第一批基础；本 RFC 固化它们共同需要的机器协议与演进方向。

目标是让一次调用提供最小充分上下文，并让后续调用可验证、可继续展开，而不是输出完整 snapshot。

## 2. 约束与非目标

- 定位的主键是 `namespace/definition`、tree selector、subtree fingerprint 与 revision；数字 path 仅在当前 revision 内有效。
- 不以 source line/column 为内部事实来源。若未来适配编辑器，行列只能由当前序列化文本临时映射。
- 默认每次命令重新读取并解析所需 snapshot/module/doc。先依靠 Calcit 的启动速度，避免常驻进程带来的缓存失效、生命周期和维护成本。
- 不以 LSP 为前置条件；只有重复解析已被基准证明确实成为瓶颈，且 LSP 的映射收益超过维护成本时，才评估薄适配层。
- 本 RFC 不改变 snapshot 的 EDN 存储形式，也不引入 workspace。

## 3. Typed result 与机器 renderer

每个命令先产生 typed result，再由 human/Cirru EDN/JSON renderer 输出；禁止从人类文本反向解析机器数据。

方向更新（2026-08-14）：Calcit 自身的 Symbol、Tag、Set、Enum/Struct 和 quoted Cirru AST 在 Cirru EDN 中可以无损表达，因此新增加的丰富机器协议应优先定义 `--format edn`，并把它作为权威 schema。JSON 的优势是 `JSON.parse`、`jq`、LSP/MCP adapter 和既有 Agent 脚本兼容，不是表达 Calcit 数据的必要条件；已有 `--format json` 契约继续稳定支持，但定位为 compatibility projection，不再反向限制 typed result。

- `--format edn` 时 stdout 只能是一个带 schema version 的 Cirru EDN value；
- `--format json` 时 stdout 仍只能是一个 JSON value；非 JSON 原生类型使用稳定 tagged encoding；
- 新字段先定义 EDN 形状，再定义 JSON 投影；
- 现有只支持 JSON 的命令渐进增加 EDN renderer，不做破坏式切换。

```cirru
{}
  :schema-version 1
  :command :query.context
  :revision |opaque-content-hash
  :data $ {}
  :diagnostics $ []
  :next $ []
```

规则：

- `--format edn` / `--format json` 时 stdout 都只能是一个 value；日志、tips 与 command echo 一律去 stderr；
- `revision` 是不透明内容 hash，不承诺可读或跨项目一致；
- EDN 缺失信息用 `nil` 或空集合表达，不依赖缺失字段或自然语言推断；JSON projection 对应使用 `null`；
- 有界列表携带 `truncated`、`next_cursor` 或可继续查询的 handle；
- 保留兼容期 `--json`，但内部等价于 `--format json`；
- 结果 schema 只能向后兼容地增加可选字段；破坏性变更升级 `:schema-version`，JSON projection 使用 `schema_version`。

### 3.1 JSON compatibility projection

JSON 只是 EDN typed result 的兼容投影，不能把 Calcit 值压扁成容易歧义的
字符串或普通数组。projection 使用稳定的 tagged shape；对象字段使用
snake_case，集合保持顺序，Set 额外携带 `set` 标签以区别普通数组：

| Cirru EDN | JSON projection |
|---|---|
| Symbol `foo/bar` | `{"$type":"symbol","value":"foo/bar"}` |
| Tag `:ok` | `{"$type":"tag","value":"ok"}` |
| Set `#{:a :b}` | `{"$type":"set","items":[...]}` |
| Quote `quote (...)` | `{"$type":"quote","value":...}` |
| anonymous Enum `:: :call a b` | `{"$type":"enum","variant":"call","type":null,"items":[...]}` |
| Struct | `{"$type":"struct","name":"...","fields":{...}}` |
| nested value | recursively apply the same mapping |
| `nil` | JSON `null` |
| empty collection | tagged empty collection, never `null` |

Implementations must include semantic fixtures for every row, including nested
values and `nil` versus empty collections. A fixture passes only when decoding
the projection reconstructs an EDN value equivalent to the source; a successful
`JSON.parse` alone is insufficient. The EDN renderer remains the reference for
round-trip tests, while JSON fixtures protect compatibility for `jq`, LSP/MCP
adapters and existing scripts.

最小语义 fixture（`typed-value/nested-v1`）如下；测试同时检查 decode 后的 EDN
等价性，而不是只检查 JSON 文本可解析：

```cirru.no-check
{}
  :value $ :: :ok
    {}
      :name 'calcit/demo
      :items $ #{} :ready
      :empty $ []
      :missing nil
```

```json
{"value":{"$type":"enum","variant":"ok","type":null,"items":[{"$type":"struct","name":"","fields":{"name":{"$type":"symbol","value":"calcit/demo"},"items":{"$type":"set","items":[{"$type":"tag","value":"ready"}]},"empty":[],"missing":null}}]}}
```

实现应为 EDN/JSON 各保留一组同构 fixture，并至少覆盖该嵌套案例、空 Set/空
List、`nil`、Quote 和带类型名的 Enum/Struct。

## 4. 统一 Definition Descriptor

query、docs、静态分析与 builtin fallback 应从同一只读描述视图组装结果：

```cirru
{}
  :id 'calcit.core/to-js-data
  :revision |...
  :kind :proc
  :source $ {}
    :scope :core
    :definition 'calcit.core/to-js-data
  :schema nil
  :inferred-type nil
  :doc |...
  :examples $ []
  :tags $ #{} :js-ffi
  :dependencies $ []
  :usages $ []
  :diagnostics $ []
```

它是 provider 组合出的只读 view，不应变成 runtime 与 query 层互相依赖的巨型数据结构。builtin 与 source-backed definition 必须落入同一结果模型。

## 5. 最小命令面

优先完善以下只读命令，而不是新增多套近似查询：

```bash
cr capabilities --format edn
cr query context <ns/def> --budget 2500 --format edn
cr query type <type-or-definition> --format edn
cr query type-at <ns/def> --path code@3.2 --format edn
```

`capabilities` 返回命令、参数/结果 schema、只读性、幂等性和支持的格式，使 Agent 不必加载所有 CLI help。

`context` 默认只返回摘要、有限代码 fragments、schema/type、有限 examples、直接依赖/引用、关联文档、诊断和下一步 handle；`--profile understand|edit|debug|document` 仅是字段预算预设。

`type-at` 应优先接受 semantic selector + expected fingerprint；数字 path 是兼容输入。查询只允许 parse、macro/preprocess 与静态 inference，不能因回答问题隐式执行 init function 或 FFI。

## 6. 何时评估 daemon / LSP

每次调用重新解析是默认架构。只有同时满足下列条件才进入实验：

1. agent-interface 基准表明解析占主要延迟，且同一项目的连续查询不能由现有 cache 消解；
2. daemon 能按 revision 可靠失效 snapshot、Git modules 与 docs；
3. editor/LSP 映射不把行号变成新的事实来源；
4. 有明确维护者承担协议兼容、进程恢复和跨平台测试。

届时优先实现 `cr serve --stdio`，复用本 RFC typed result。stdio 使用一行一个
请求、一行一个响应的 framing；每一行必须是一个完整 EDN value，或在握手后是
一个完整 JSON value，禁止把日志写入 stdout。客户端首先发送：

```cirru.no-check
{}
  :protocol |calcit-agent/1
  :format :edn
  :request-id |hello-1
```

服务端响应同样带 `:request-id` 与最终选择的 `:format`。当前实现应优先选择
EDN；若客户端只声明 `:json`，服务端选择 JSON；无法满足时返回结构化错误并
关闭会话。后续请求使用 `:request-id`、`:command`、`:params`，响应使用
`:ok`、`:result`、`:diagnostics`；错误使用 `:ok false`、`:error {:code ...
:message ... :details ...}`，EDN/JSON 两种格式字段语义完全一致。stdio 同样
遵守“一次响应一个 stdout value”，stderr 才能承载日志和进度信息。

协议 fixture 必须覆盖 EDN 握手、JSON 握手、格式不支持错误、请求/响应关联和
上述 typed-value projection。LSP/MCP 只是其上的薄映射；LSP 的 document symbol、
references、hover、versioned diagnostics 与 workspace edit 可以借鉴，但内部身份
仍是 definition/tree 语义。

握手 fixture 至少包含两组等价请求：一组声明 `:format :edn` 并逐行传输 EDN，
另一组声明 `:format :json` 并逐行传输 JSON；两组都必须得到相同语义的
`:request-id`、`:ok`、`:result`/`:error` 响应。

## 7. 验收

- Cirru EDN stdout 可无损 round-trip Symbol、Tag、Set、Quote、Enum/Struct，且版本、命令、revision、data、diagnostics 一致；
- compatibility JSON stdout 仍可由 `JSON.parse` 直接解析，已有调用方不被破坏；
- Agent 能用一次 `context` 和一次可选展开完成一个 definition 的理解；
- 对相同 revision 的重复只读调用结果稳定；
- `yarn check-agent-interface` 覆盖并记录耗时、stdout bytes、失败原因；
- 性能数据而非直觉决定是否开始常驻服务或 LSP。
