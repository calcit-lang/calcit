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

这套规则参考 Rust API 中「类型、构造、方法各有位置」的可预测性，但不照搬 snake_case 或 `Type::method` 语法。Calcit 保留有意义的 kebab-case、谓词 `?` 和作用标记 `!`。具名 Enum 定义同时是 schema 中的类型名和构造值时的可调用入口：`Option :some value` 与 `Result :err error` 中的类型名都必须解析到对应 core 定义。`%some/%none/%ok/%err` 是旧构造 helper，已有 [#1420](https://github.com/calcit-lang/calcit/issues/1420) 的 fix 可迁移；本页只推荐直接构造。

## 表层命名矩阵

| 角色 | 首选形态 | 命名与类型依据 | 不作为新代码入口 |
| --- | --- | --- | --- |
| 类型、trait、Enum 变体 | `Person`、`Option`、`Result`；`:some`、`:err` | 类型名用 PascalCase；variant 用 tag，由定义验证 payload；`Option<T>` / `Result<T,E>` 写进 schema | 把 `%some` 当作类型或把裸 `Option` 当作已构造的值 |
| 构造 | `Person :name name`、`Option :some value`、`Result :err error`；集合字面量 `[]` / `{}` | 调用已知名义定义，保留 `T -> Option<T>` 等具体关系，不以 `%` 前缀猜构造类别 | `%{}`、`%::` 的静态可替代场景；`%some/%none/%ok/%err` |
| 接收者方法 | `value .unwrap`、`text .includes? fragment`、`items .get index` | 接收者类型决定可用方法与返回类型；名称用 kebab-case，布尔谓词以 `?` 结尾；`query type` 应显示 `proven` | 在业务代码直接使用 `option:unwrap`、`&str:includes?` |
| 无接收者函数 | `parse-float source`、导入的 `module/function` | 行为无自然接收者，或显式跨模块复用；名称用 kebab-case；`parse-float: String -> Result<Number,String>` | 为已有 typed receiver 另造平行的公开 `type:verb` 函数 |
| 内部 helper 与 primitive | `option:unwrap`、`&str:includes?`、`&list:count` | `:internal` 标签与实现链接表明它们是 core/lowering 细节；`&` 保留给 runtime/backend primitive | 把内部路径写成应用文档中的首选 API，或仅凭名字推断其类型安全性 |

`?` 表示返回布尔判断，`!` 只用于确有作用或特殊控制语义的公开名字；不要机械地给每个动词添加后缀。模块名和普通函数继续用 kebab-case。公开名称是否是方法由类型契约和解析证据决定，不由字符串里是否有 `:` 或 `&` 决定。内部 helper 与 primitive 暂有不同实现命名；它们不是两套公开语言风格，不承诺用户可依赖其拼写。下一步先迁移公开调用，再根据真实编译器重复逻辑决定是否调整内部名字，避免为了表面一致重做 lowering。

### English reference for Agents

| Role | Preferred public form | Legacy or implementation form |
| --- | --- | --- |
| Type and variant | `Option<T>` with `:some` / `:none`; `Result<T,E>` with `:ok` / `:err` | A bare type name is not a constructed value. |
| Constructor | `Option :some value`, `Result :err error`, `Person :name name` | Migrate `%some`, `%none`, `%ok`, `%err` with `core-nominal-constructor-v1` when the call is statically resolved. |
| Receiver method | `value .unwrap`, `text .includes? fragment` | `option:unwrap value` and `&str:includes? text fragment` are implementation paths, not preferred application calls. Preview proven Option rewrites with `calcit fix --rule core-option-method-v1 --format edn`. |
| Module function | `parse-float source`, imported `module/function` | Use when there is no natural typed receiver; do not introduce a parallel public `type:verb` spelling. |
| Internal helper / primitive | No public spelling promised | `option:unwrap` is a core helper; `&str:includes?` and `&list:count` are runtime primitives. Trace them for implementation, not API discovery. |

The public, human-facing form is distinct from macro-expanded core definitions and JS/native/WASI lowering names. Type schemas and receiver resolution—not punctuation alone—establish a method's contract. Keep Calcit's kebab-case, `?` predicates, and meaningful `!` effects; Rust is a model for clear roles, not a mandate to copy Rust syntax. Deprecated constructors may remain during migration, but Agent-facing examples should show one preferred spelling. Ambiguous or shadowed calls require manual review rather than an unproven rewrite.

`Option` / `Result` 等名义定义即使在 core 元数据中带 `:internal`，也可以是公开的 schema 类型和直接构造入口；判断是否推荐给应用要看其**角色和调用契约**，不能只按一个 tag 或定义路径过滤。

| 遇到的写法 | 新代码首选 | 迁移与限制 |
| --- | --- | --- |
| `%some value`、`%none`、`%ok value`、`%err error` | `Option :some value`、`Option :none`、`Result :ok value`、`Result :err error` | `core-nominal-constructor-v1` 只改写解析到 core 且保序可证明的调用；被遮蔽或当函数值传递时人工确认 |
| `option:unwrap value`、`option:unwrap-or value fallback` | `value .unwrap`、`value .unwrap-or fallback` | `core-option-method-v1` 只自动改写接收者有编译器证明的调用；core 空 Option 可由具体 fallback 补足类型，其他开放类型、未知 macro、函数值引用仍需人工确认 |
| `result:unwrap-or result fallback` | `result .unwrap-or fallback` | `core-result-method-v1` 只改写目标方法已证明的调用；core `:err` 可由具体 fallback 确定成功类型，同时保留错误类型；开放 Result、`:ok` 动态值和遮蔽调用仍需审阅 |
| `option:some?` / `option:none?`、`result:ok?` / `result:err?` | `value .some?` / `.none?`、`value .ok?` / `.err?` | 沿用对应 Option / Result 方法规则，仍须证明接收者名义类型及同一 core 方法；裸 Dynamic 不能借 Bool 返回值绕过类型检查 |
| `&str:includes? text fragment`（容易被误记为 `&string:include?`） | `text .includes? fragment` | 当前内部定义的准确拼写是 `&str:includes?`；`.contains?` 对 String 检查索引，不能把两者机械互换 |
| 静态 `%:: Enum :variant payload`、`%{} Struct (:field value)` | `Enum :variant payload`、`Struct :field value` | 只有类型定义能静态解析、variant/字段契约可证明时才自动修复；动态 prototype 边界仍可显式使用低层形式 |

这张对照表只规定**公开源码的首选写法**，不是要求一次删除所有内部实现名。内部函数即使仍出现在 `query type` 的定义路径中，也不得反向成为 Agent 推荐的应用代码。

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

对 Option 的旧 helper，可用 `calcit fix --rule core-option-method-v1 --format edn` 预览 `.some?` / `.none?` / `.unwrap` / `.unwrap-or` 迁移。只有接收者方法解析为 `proven` 且实现确实指向相同 core helper 时才提供自动改写；已核实单次保留调用的 core `let`、`cond`、`do`、`fn`、`assert=` 宏允许通过，其他宏仍需审阅。已证明来自 core 的空值 `%none` 或 `Option :none` 即使起初显示 `Option<Dynamic>`，也可由具体 fallback 推出 payload 类型，再检查方法分派；普通开放接收者、函数值或无来源映射的表达式仍不能自动改写。应用后重复预览应为空，并运行严格类型检查与项目测试；不把 String 或其他内部函数类推为同一个迁移。

Result 的 `result:ok?` / `result:err?` / `result:unwrap-or` 沿用单独的 `core-result-method-v1` 规则，不新增顶层 CLI 入口。只有编译器能证明同一方法契约才自动迁移；已证明来自 core 的 `%err` / `Result :err` 不携带成功值，因此允许具体 fallback 绑定成功类型，同时仍检查错误类型。成功值已有具体类型的 core `%ok` 可按同一方法契约迁移；普通开放 Result 和 `:ok` 的动态成功值不能凭 fallback 收窄，具名构造缺少精确源码证据时也保留审阅。旧 helper 作为 core 实现暂留；公开文档只推荐 `.ok?` / `.err?` / `.unwrap-or`。在真实消费者完成迁移、Agent 查询和升级文档统一首选写法、经过至少一个发布窗口且 JS/native/WASI 相关测试通过之前，不删除旧 helper。其他 `result:*` 逐项确认语义与迁移证据，不跟随批量废弃。
