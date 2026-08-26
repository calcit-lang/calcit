---
title: "Type Guidance"
summary: "Dynamic 审计、Option/Result 组合、嵌套数据访问和类型化 Enum 构造"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "dynamic audit"
  - "option result"
  - "typed enum"
id: core/features/type-guidance
parent: core/features
---

# Calcit 类型使用指南

## Dynamic 是边界，不是默认多态

`Dynamic` 适合 JS FFI、框架开放数据、宏和确实无法提前知道的外部输入。普通函数不要用多个 `Dynamic` 表示“它们应该是同一个类型”：输入和返回关联时用 `:generics` 与 TypeVar；只需要能力时用 trait 与 `:where`；同质集合写出元素类型；有限异构数据定义为 Enum；可缺失值使用 `Option<T>`，带失败信息使用 `Result<T, E>`。

每次执行和编译会在 stderr 输出 Dynamic 用量提示。它是趋势信号，不会替代具体路径检查：

```bash
calcit analyze check-types --summary-only
calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --format json
```

## 用原生 quality gate 阻止类型债务回归

新项目直接要求所有发布指标归零：

```bash
calcit calcit.cirru analyze quality
```

存量项目先审阅当前结果并写入 baseline，随后在 CI 中只执行比较命令：

```bash
calcit calcit.cirru analyze quality --write-baseline config/calcit-quality.json
calcit calcit.cirru analyze quality --baseline config/calcit-quality.json
```

门禁同时覆盖未完整类型、unresolved Dynamic、未迁移的 nil/Optional、deprecated calls 与显式 `unsafe-coerce`。`unsafeCoerce` 是独立的 host-boundary 预算，不能当作已解决的 Dynamic。原生 v2 baseline 按 definition 保存预算，新债务不能被其他 definition 的改善抵消；旧 v1/扁平 baseline 仍只执行它们原来的八项指标，审阅后重新生成 baseline 才会启用这个新预算。`--write-baseline` 只用于明确审阅后的更新，不应作为每次 CI 的前置步骤。需要机器读取时追加 `--format json`，失败时 stdout 仍是单个 JSON，进度与错误摘要写入 stderr。

baseline 是已提交的机器生成工件。为避免它的 JSON diff 默认占据 PR 视图，可在项目根目录的
`.gitattributes` 加入与 `yarn.lock` 相同的生成物标记：

```gitattributes
config/calcit-quality.json -diff linguist-generated
```

这不会忽略或删除 baseline；GitHub 只会默认折叠其 diff。`-diff` 同时会让本地 Git 默认不生成
该文件的文本 diff；如果本地审阅优先，可省略 `-diff`，只保留 `linguist-generated`。更新
baseline 的 PR 仍应展开文件并按 definition 审阅预算变化。

## Option / Result 组合

优先让 `Option` / `Result` 的方法表达类型流，而不是逐层 `unwrap` 或调用
`option:*` / `result:*` 的函数形式：

```cirru.no-check
user .and-then $ fn (user)
  (get user :profile) .and-then $ fn (profile) (get profile :name)

loaded .and-then $ fn (value) (validate value)
```

备用来源使用 `.or-else`。`.unwrap-or` 只用于确实需要默认值的终点，`.map` 用于同步转换，`.and-then` 用于下一个仍可能失败的操作。保留 `Option` 本身能让类型系统持续检查缺失路径；不要为了集合判断而把它解成 `nil`。

多个连续的 Option 步骤可以实验性使用 `option:let`。Result 流程直接使用接收者
`.and-then`，让错误类型的转换保持可见：

```cirru.no-check
(path .read-file) .and-then $ fn (content)
  (parse-data content) .and-then $ fn (data)
    save-data data
```

文件路径上的 `.read-file`、`.read-dir` 与 `.write-file` 返回 `Result<...,String>`；
底层同名函数仍保留抛错行为以兼容旧代码。预期 I/O 失败时优先使用方法形式。
这些文件效果支持 native 与生成的 JavaScript；WASM 尚未提供宿主文件效果。

`option:let` 的 body 必须继续返回 `Option`。普通组合函数仍以接收者方法
作为公开形式，`option:*` / `result:*` 直接函数调用主要保留给 core lowering。

## get-in / update-in

`get-in` 是可能失败的开放数据访问，返回 `Option<T>`。不要用它绕过 Struct 字段检查；Struct 路径应使用 `(:field value)`，字段可缺失就把字段声明为 `Option<T>`。

`update-in` 的 updater 接收 `Option<T>`。对缺失值给默认值或明确返回 `%none`，不要无条件 unwrap：

```cirru.no-check
update-in data ([] :settings :retries)
  fn (current) (current .unwrap-or 0)
```

## Enum 构造

Struct 也支持同样的类型化头部调用：

```cirru.no-check
defstruct Profile (:name 'String)
  :bio $ :: 'Option 'String

Profile :name |Ada
```

参数必须是 `:field value` 对，必填字段不能省略；末尾声明为 `Option<T>`
的字段可以省略，Calcit 会补成 `%none`。需要显式控制所有字段或构造部分值时，
继续使用 `%{} Profile ...` / `%{}? Profile ...`。

已知 Enum 定义时使用头部调用：

```cirru.no-check
Option :some value

Result :err message
```

Calcit 会根据 Enum 定义检查 variant 和 payload，并在预处理阶段生成命名构造。`%::` 保留给显式 prototype、动态跨模块构造和兼容旧代码的边界。
