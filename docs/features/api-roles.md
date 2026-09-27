---
title: "Calcit API 命名角色"
summary: "区分类型名、构造器、方法、命名空间函数和内部实现；让人类与 Agent 找到推荐调用入口"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "API naming"
  - "constructor naming"
  - "method naming"
entry_for:
  - "calcit query type"
  - "calcit query def"
id: core/features/api-roles
parent: core/features
related:
  - core/features/type-guidance
  - core/run/query
  - core/agent
leads_to:
  - core/run/query
---

# Calcit API 命名角色

调用前先判断角色，再找拼写：类型名描述值的形状，构造器创建值，方法以已知类型的值为接收者，命名空间函数执行独立操作，内部函数只是编译器或 core 的实现入口。同一操作在内部可能有多种表示，不代表每种表示都应成为用户的首选 API。

这套规则参考 Rust API 中「类型、构造、方法各有位置」的可预测性，但不照搬 snake_case 或 `Type::method` 语法。Calcit 保留有意义的 kebab-case、谓词 `?` 和作用标记 `!`。具名 Enum 定义同时是 schema 中的类型名和构造值时的可调用入口：`Option :some value` 与 `Result :err error` 中的类型名都必须解析到对应 core 定义。`%some/%none/%ok/%err` 是旧构造 helper，后续按 [#1420](https://github.com/calcit-lang/calcit/issues/1420) 的 fix 迁移。

| 角色 | 当前代表 | 类型或签名证据 | 推荐用法 | 其他入口的定位 |
| --- | --- | --- | --- | --- |
| nominal 类型 | `Option`、`Result`；`:: 'List 'Number`、`:: 'Map 'Tag 'Number` | `Option<'T>`、`Result<'T,'E>`、`List<Number>`、`Map<Tag,Number>` | 用类型名写 schema、`query type` 查方法 | core 定义可能带 `:internal`，不表示名义类型不可用于公开 schema；具名 Enum 定义也能直接构造值 |
| 构造器 | `Option :some value` / `Option :none`、`Result :ok value` / `Result :err error`；`[]`、`{}`、字符串字面量 | `Option :some` 保留 `T -> Option<T>`；`Result :ok` 保留 `T -> Result<T,E>` | 具名 Enum 直接调用定义，随后让推断保留具体类型 | `%some/%none/%ok/%err` 是待迁移的旧 helper；`%::` 留给动态 prototype 和兼容边界 |
| 实例方法 | `option .unwrap`、`result .unwrap-or fallback`、`text .includes? fragment`、`items .get index`、`table .get key` | `Option<Number>.unwrap: () -> Number`；`String.includes?: (String) -> Bool`；`List<Number>.get: (Number) -> Option<Number>`；`Map<Tag,Number>.get: (Tag) -> Option<Number>` | 接收者类型已知、`query type` 显示 `proven` 时优先使用 | `query type` 中显示的 `calcit.core/&...` 是实现定义链接，不是与 `.method` 平级的推荐调用 |
| 命名空间函数 | `parse-float source`、用户模块的 `ns/function` | `parse-float: String -> Result<Number,String>` | 操作本身无接收者时使用；项目函数按模块导入 | 不能仅因名字里有 `:` 就判定为用户公开 API |
| 内部实现 | `option:unwrap`、`&str:includes?`、`&list:count` | 内部定义带 `:internal`；底层签名可供定位 | 通常只在维护 core 或明确的开放边界使用 | 先查接收者的 `.method` 和类型证据，避免绕开推断或把内部拼写传播到应用 |

## 可运行的角色示例

`Option` 是名义类型定义；加上 variant 后，同一定义也能直接构造值。类型表达式仍写在 schema 中，不能把 `Option` 裸名当成已构造的值：

```cirru
assert= 3 $ .unwrap $ Option :some 3
```

方法在具体接收者上保留类型关系，失败行为也属于该方法的契约：

```cirru
assert= 3 $ (Option :some 3) .unwrap
```

无接收者的解析操作保持命名空间函数形式，其返回值是 Result：

```cirru
assert= (Result :ok 3) $ parse-float |3
```

String 的 `.includes?` 检查子串；`.contains?` 在 String 上检查**字符索引**是否有效，不能只凭英文词形把两者当同义词。List 和 Map 的 `.get` 都返回 Option：

```cirru
do
  assert= true $ |abc .includes? |b
  assert= (Option :some 2) $ ([] 1 2 3) .get 1
  assert= (Option :some 2) $ ({} (:x 2)) .get :x
```

下面是内部 primitive 的定位示例，不是应用代码的推荐写法；它和上面的 `.includes?` 在这个输入上结果相同，但应用应先使用方法：

```cirru
assert= true $ &str:includes? |abc |b
```

## Agent 查询顺序

先查类型再选方法，不要从模糊搜索到的内部定义名猜调用形式：

```bash
calcit query type ":: 'Option 'Number"
calcit query type "'String"
calcit query type ":: 'List 'Number"
calcit query type ":: 'Map 'Tag 'Number"
calcit query def 'calcit.core/Option' --format edn
calcit query def 'calcit.core/%some' --format edn
calcit query context 'calcit.core/option:unwrap' --format edn
```

`query type` 的 `proven` 表示当前类型能证明该方法的调用契约；`open` 或 `ambiguous` 不是类型安全的肯定结论。方法旁边的 definition path 用于追踪实现。再用 `query def/context` 读取 `:constructor` 或 `:internal` 标签、schema 与示例；需要机器处理时优先用 `--format edn`。项目代码的类型证据可用 `query type-at` 或 `query context` 查看。

真实项目中，Quamolit 的 `quamolit.gpu-scalar-program/first-slot` 以 `get slots 0` 得到 `Option<BoundScalar>`，再调用 `.unwrap`；Timegrass 的 `app.server/main!` 对 `parse-float raw` 的 `Result<Number,String>` 调用 `.unwrap-or 11009`。这两条路径均可在各自 Snapshot 上用 `query context` 找到，再用 `query type ":: 'Option ..."` 或 `query type ":: 'Result ..."` 检查方法契约。它们说明推荐入口应由返回类型决定，而不是由内部函数名字决定；不要求改变这些项目的源码。

`%some/%none/%ok/%err` 从 0.25.0 起标记弃用；在确认真实项目能迁移后，最早于 0.26.0 移除。先用 `calcit fix --rule core-nominal-constructor-v1 --format edn` 预览，再带预览返回的 `--expect-revision` 应用。规则只自动改写编译器已解析到 core、实参数量匹配且处在可证明保序的源码调用；把 helper 当函数值、局部遮蔽或跨未证明的 macro 边界时需人工确认，不会插入 Dynamic 或 `unsafe-coerce`。String 的 `.contains?`（索引）与 `.includes?`（子串）仍有命名歧义，但 `.contains?` 属于跨容器 trait，不适合在本试点机械重命名。
