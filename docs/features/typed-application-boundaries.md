---
title: "业务数据的类型边界"
summary: "用 Diary 五类最小用例区分静态契约、受检 decode 和应用迁移责任"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "typed application boundary"
  - "Diary migration"
---

# 业务数据的类型边界

在开放输入处 decode，一旦得到具名值便把它传给有明确 schema 的业务函数。
`try-parse-cirru-edn-as` 检查持久化文本，`try-decode-map-as` 检查已经求值的值；
两者的 `Result :ok` payload 是受检结果，业务代码应使用这个结果，而不是继续传递原始输入。
`assert-type` 是编译期检查，不会检查运行时网络数据，也不会把 Map 转成 Struct。

## 五类迁移案例

下面来自 Diary 的[固定公开源码](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru)。
对应最小表达式保存在 `tests/fixtures/def-value-schema.cirru` 的
`app.main/verify-login-decode :tests`，用 `:diary-boundary` 选择。
它们提取类型契约，不复制完整应用或用户数据。

| 案例与原始契约 | 分类 | 最小正反例与应用责任 |
| --- | --- | --- |
| [存储](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru#L1738-L1790)：`String -> Database`，typed parse 后允许 legacy normalization | 合法的持久化开放边界 | `persisted-nested-value` 检查 `Map<Tag,List<Number>>` 的深层值，错误元素返回带 `[1]` 的错误；应用仍需验证 normalization、备份、临时文件与原子替换 |
| [网络 patch](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru#L157-L180)：`Dynamic -> Unit`，decode 成功后却丢弃 payload、保存原 Map | 开放消息边界与应用结果使用错误 | `checked-patch-projection` 提取 `ClientStore` 的 count/color 契约，成功候选成对保留原 Map 与受检 nominal projection；错误类型和缺少字段返回错误，原候选不变。实际 patch op、Ref 提交与重连仍由应用验证 |
| [members producer](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru#L2187-L2198) 返回 `Map<Number,String>`，[consumer](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru#L1309-L1339) 要求 `Map<String,String>` | 应用契约矛盾，不是缺少自动转换 | `member-key-contract` 接受 Number key、拒绝 String key 契约；不要自动 stringify，也不要扩大成 Dynamic |
| [登录恢复](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru#L208-L220)：localStorage 文本直接 parse 后 dispatch，操作契约是 `List<String>` | JS host / 持久化开放边界 | `restored-credentials` 接受字符串列表、拒绝第二项 Number；列表长度、合法操作与无效输入不 dispatch 是应用额外契约 |
| [组件 state](https://github.com/TopixIM/diary/blob/787fb808579a8ee42197a2bc9eb2f896ebcddf26/calcit.cirru#L715-L771)：初始化为 `LoginState`，却声明 `Map<Tag,String>` | 应用 schema 错误，以及初始化契约漏检 | `nominal-component-state` 保留具名字段及不可变更新；已有严格负例将 initializer/alias 改成 Map schema，编译期拒绝，不能用空 trait 代替数据证明 |

## 编译器证明与输入校验各管一层

具名 initializer 的真实类型现在沿不可变 `def` 与跨 namespace alias 检查；错误的
`LoginState -> Map<Tag,String>` 和 members key schema 会产生 `E_SCHEMA_DEF_MISMATCH`，
不需再靠读取处增加 Dynamic/unsafe fallback。修正声明为具名 schema 后，
`:username state` 直接使用具名字段契约。错误声明仍在 native 执行和 JS codegen 之前拒绝。
这种共享检查不替代网络、文件和 localStorage 的运行时 decoder。

静态 Map 访问用 `get` 并处理 `Option`；`:field value` 表示已声明 Struct 的必需字段。
业务层不要为了避开缺失 key 或错误 key 类型而混用两者。

## Ref decode 与原 alias

`try-decode-map-as raw (:: 'Ref 'Number)` 检查当前 payload；成功结果中的
`Ref<Number>` 是一个新 cell，不是原 Ref 的原地类型转换。修改原 cell 或其 alias
不会改变受检结果，修改受检结果也不会改变原 cell。失败返回带 `.value` 路径的错误，
原 cell 的值保持不变。

原来的 `Ref<Dynamic>` 及其 alias 仍然开放。即使位于 decode 成功分支，
`assert-type alias (:: 'Ref 'Number)` 也没有独立证明；Agent 不应添加断言、unsafe
或 schema 来假装原 alias 已安全。需要保留原 Ref 身份时，应人工确认受检写入与
共享 alias 的业务契约，而不是自动把旧 cell 换成新 cell。

这些行为由 `try-decode-map-as` 自身的 `:tests` 固定，既有 runner 读取同一表达式
回放 native/JS，并检查原 alias 的 proof preview 拒绝且不写回。

## Trait 与 derive 的选择

这五例不需要新增 derive 语法：字段、容器元素、nominal initializer 与 decoder 已能表达必要契约。
成员 key 的业务选择、patch 提交策略和历史数据 normalization 不能从字段类型自动派生。
空 trait 仅证明 nominal capability，不证明 Dynamic payload 的形状。

需要公共行为时，先用现有 `deftrait`、`defimpl`、`impl-traits` 及显式 schema/`:where`
约束连接手写实现；复用一个受检边界函数，业务函数只接收它的具体返回类型。
后续自动派生只能生成字段约束可确定性证明的实现，并保留定义来源、冲突和失败位置；
不能自动授权 FFI、补 unsafe、选择默认值或跳过 decoder。

## Agent 验证闭环

```bash
calcit tests/fixtures/def-value-schema.cirru query def app.main/verify-login-decode --format edn
calcit tests/fixtures/def-value-schema.cirru test --tag diary-boundary --require-match
calcit src/cirru/calcit-core.cirru test calcit.core/try-decode-map-as --tag ref-alias-boundary --require-match
yarn check-strict-default
```

现有 runner 从同一组 `:tests` 读取 AST，用正常 JS codegen 回放；严格负例继续覆盖
initializer、alias 与 Map key schema，检查 source 字节不变。JSON 仅用于 Node runner 的
显式互操作读取，不改变用户查询默认格式。

### 限制

- 这些最小用例不证明完整 Diary 迁移、浏览器 localStorage、网络重连或文件原子替换已经完成。
- `List<String>` 不证明恰好两个元素，credentials 的长度和业务有效性需另行检查。
- 这组 nominal/map/Ref decode 用例只验证 native/JS；`try-decode-map-as` 的 WASM 回放明确 unsupported 且不生成产物，不宣称三后端 parity。
