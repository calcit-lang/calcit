---
title: "Calcit API 命名角色"
summary: "区分 API 角色；按家族给出命名决策、类型与失败契约、等价迁移和暂缓边界"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "API naming"
  - "constructor naming"
  - "method naming"
  - "API vocabulary"
  - "命名决策"
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

已知 `List<T>` 和 `String` 的 `.first`、`.last`、`.nth` 使用同一接收者证明推导 `Option<T>` 或 `Option<String>`；`calcit query type ":: 'List 'Number" --format edn` 可查看参数、返回值和 `proven` 状态。List 的 `.nth` 仍按已有规则标为兼容名、推荐 `.get`，证明状态不改变 API 角色。开放接收者或 Enum 的异质 payload 不能仅凭方法名得到具体成员类型；查询保持开放证据，不把 `Dynamic` 伪装为已证明的 `T`。

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
| `&str:includes? text fragment`（容易被误记为 `&string:include?`） | `text .includes? fragment` | 当前内部定义的准确拼写是 `&str:includes?`；旧 `.contains?` 对 String 检查索引，其首选名为 `.contains-index?`，不能把索引与子串查询机械互换 |
| 静态 `%:: Enum :variant payload`、`%{} Struct (:field value)` | `Enum :variant payload`、`Struct :field value` | 只有类型定义能静态解析、variant/字段契约可证明时才自动修复；动态 prototype 边界仍可显式使用低层形式 |

这张对照表只规定**公开源码的首选写法**，不是要求一次删除所有内部实现名。内部函数即使仍出现在 `query type` 的定义路径中，也不得反向成为 Agent 推荐的应用代码。

### 索引单位：普通索引与显式 byte 单位

String 的 `.len`（兼容 `.count`）、`.get/.nth/.slice` 与 `.find-index` 使用 Unicode 标量单位；查找返回 `Option<Number>`，不是 Rust `str::find` 的字节偏移。Calcit 借鉴语义清晰的命名，不照搬会破坏自身索引一致性的底层表示。`.includes?` 判断子串，`.contains-index?`（兼容 `.contains?`）判断索引存在，不因统一索引单位而互换。协议长度使用显式 `&str:utf8-byte-count`，不可混入普通索引。示例和边界见 [String](../data/string.md#子串搜索索引)；其他 API 族的重命名仍由 #1452 逐项决策，本修复不增加别名或 fix 规则。

## 逐族命名决策（0.27–0.28）

这是 [#1452](https://github.com/calcit-lang/calcit/issues/1452) 的实施契约，不是“以下新名字已经可用”的 API 清单。**保留**表示当前首选不变；**目标**表示本轮确定的新拼写、仍待对应 issue 实现；**内部**表示只保留可追溯的实现角色；**暂缓**表示已有语义或类型问题尚不能安全定名。Agent 生成代码应先查当前版本的 `query type/def`，不得调用尚未实现的目标。

表中的签名包含 receiver；`T/U/K/V` 是保留关系的类型参数，不是 Dynamic。只有旧新入口的来源、参数/返回类型、失败行为和求值顺序均能证明等价，才允许自动迁移。这里的迁移分类不增加 CLI flag、命名 linter 或第二份公开 API registry。

### 词法与职责

- 类型/trait 用 PascalCase；值、函数、方法、模块、字段用 kebab-case；variant 继续使用 `:some/:err`。保留 `[]/{}/#{}`、算术/比较运算符、`defn/let/->/->>` 等结构化 Lisp 表达，不把宏改成 Rust 的 `name!` 语法。
- 返回 Bool 的判断保留 `?`，优先 `empty?`、`contains-key?`，不额外套 `is-…?`。`Option<Number>` 查找结果不是谓词；名字不能代替类型证明。
- 实例操作优先 typed `.method`；没有自然 receiver 时保留普通模块函数 `alias/name`，不再并列增加公开 `type:verb` 别名。
- 参考 [Rust 命名指南](https://rust-lang.github.io/api-guidelines/naming.html) 的角色划分和语义词汇，不照搬 snake_case、借用/所有权对应的 `as_/into_` 或惰性 iterator。Calcit 集合操作返回新值；普通 String 索引是 Unicode 标量，不是 Rust 字节偏移。
- 本轮目标是让入口与语义更可预测；不宣称仅改名就能证明 LLM 正确率提升。验收使用实际查询、迁移和消费者运行，不引入新的评分系统。

### 谓词与成员查询

实施任务：[#1454](https://github.com/calcit-lang/calcit/issues/1454)。成员判断选用现有 `.includes?`；**不把旧 `.contains?` 原地改成另一种含义**，也不安排先移走旧义、再复用同名的二次迁移。

| 当前入口与签名/行为 | 决策与目标签名 | 迁移边界 |
| --- | --- | --- |
| `some?: T -> Bool`，仅排除 nil；Option none 也为 true | **已提供** `non-nil?: T -> Bool`；`some?` 暂留兼容 | 等价改名，不自动改成 Option `.some?`；WASM 已以类型证据修复 false/0 误判，fix 仅改写 compiler-resolved core 引用 |
| Option `.some?/.none?: Option<T> -> Bool`；Result `.ok?/.err?: Result<T,E> -> Bool` | **保留**，判断名义 variant | 空值、false 和 nil payload 不改变 variant；既有 helper 退场条件仍适用 |
| List/String `.contains?: (receiver, Number) -> Bool`，检查索引 | **已提供** `.contains-index?`，签名与原有错误行为不变 | List 的负数/小数/非有限索引返回 false；String 的负数/小数报错、越界返回 false。只迁移已证明的接收者；它不是元素或子串查询，可显式预览 `core-predicate-method-v1` |
| Map `.contains?: (Map<K,V>, K) -> Bool`；`.includes?: (Map<K,V>, V) -> Bool` | **已提供** `.contains-key?`、`.contains-value?`，分别接收 K/V | 即使 K 与 V 同型，两种命题也不同；仅用显式 `core-predicate-method-v1` 迁移已证明的 core 方法 |
| Struct `.contains?` 检查字段；Enum `.contains?` 检查 tag 与 payload 位置 | Struct **已提供** `.contains-field? Tag -> Bool` 与 `contains-field? Struct Tag -> Bool`；Enum **已提供** `.contains-index? Number -> Bool` | Struct 新接口仅以 Tag 字段名承诺跨目标一致；旧底层 `&struct:contains?` 在 native/JS 还接受 String/Symbol，但不据此扩大新接口类型。Enum 的 0 是 tag，1 起是 payload；新方法仅接受非负有限整数，旧 `.contains?` 会将范围内小数误判为存在，不可无条件自动迁移。自定义 `Contains` 仍需单独证明 |
| String `.includes?: (String, String) -> Bool`；List/Set `.includes?: (C<T>, T) -> Bool` | **保留** `.includes?`；Set `.contains?` 的同义入口迁到 `.includes?` | String 是子串，其余为成员；Set 别名只在证明实际实现后迁移，不全局替换 Contains trait |
| `round?/.round?: Number -> Bool`，有限且无小数部分 | **已提供** `integer?/.integer?` 为首选；旧名暂留兼容 | 新入口复用已验证语义：NaN/±Infinity false，-0 true，非零小数 false；近零/Infinity 的旧行为变化见 [升级说明](../run/upgrade.md#整数谓词的跨目标语义修复)。Bool 不等于 Int refinement 证明；显式 `core-integer-predicate-v1` 迁移 reader 解析出的内建 `round?` 调用，以及已证明同实现、同契约的 Number `.round?`；开放接收者和用户方法不自动改写 |
| `every?` / `any?`，predicate 返回 Bool，短路；空集分别 true/false | **目标** `all?` / **保留** `any?`，参数顺序和短路不变 | 先补足已有支持的 receiver/callback 类型关系；不凭当前宽 schema 承诺所有容器，或凭改名新增方法 |
| `nil?/empty?/blank?/starts-with?/ends-with?/even?/odd?` | **保留**现有签名和命题 | 空白不等于空串；其他已有明确类型谓词同样不为相似拼写强改 |

`Contains` 不能直接别名成一个新 trait：它目前横跨索引、键、字段与成员。#1454 先为上述命题建立具体签名，逐一迁移 builtin 与已定位的自定义 impl；旧 `Contains` bound 和具名 trait-call 在兼容窗口保持原契约。泛型接收者有唯一 `where T: Contains` 来源时，调用应绑定该 trait，而非仅按实例上同名方法查找；具体接收者同时实现多个同名 trait 时，仍需显式 `&trait-call` 消歧。`core-predicate-method-v1` 仅覆盖已证明的 builtin receiver 和同一实现，普通用户自定义同名方法及 trait-bound 泛型调用不自动改写。不得把泛型 `Contains<T,K>` 草率替换成更宽 Dynamic 或猜测性的 trait 联集。

前置缺陷不能由改名掩盖：WASM 的 false/0 误判已经改为依据静态类型证据 lowering；具体参数及直接调用的泛型 helper 会单态化，无法证明类型的开放导出或一等函数边界则以 `E_WASM_NIL_TYPE_EVIDENCE` 拒绝。native、JS、core WASM 与 WASI Component 共享同一组 Calcit 定义测试；详细迁移边界见[升级说明](../run/upgrade.md#wasm-的-nil-类型证据)。在此基础上，`non-nil?` 已成为首选名字，旧 `some?` 保持同义兼容；`core-non-nil-predicate-v1` 只做已解析 core 引用的等价改名。`round?` 的近零/无穷差异已由共享测试修复，`integer?/.integer?` 复用该语义；函数及 Number 方法的显式迁移规则见 [fix 文档](../run/fix.md)。List/String 索引和 Map 键/值的新方法已提供，首批 builtin 的显式 `core-predicate-method-v1` 也见 fix 文档；Struct/Enum 与 `Contains` trait 的迁移仍由 [#1482](https://github.com/calcit-lang/calcit/issues/1482) 跟踪。

下面是**当前旧契约的反例**，说明为什么不能只凭词形替换；对应 definition `:tests` 会随等价迁移一起保留这些断言：

```cirru
do
  assert= true $ non-nil? $ Option :none
  assert= false $
    Option :none
    , .some?
  assert= true $
    [] 10 20
    , .contains? 1
  assert= false $
    [] 10 20
    , .includes? 1
  assert= true $
    {} $ :key :value
    , .contains? :key
  assert= false $
    {} $ :key :value
    , .includes? :key
```

### 集合长度、组合与遍历

实施任务：[#1455](https://github.com/calcit-lang/calcit/issues/1455)。参考 [Rust fold/reduce](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.fold) 对“有初始值”和“无初始值”的区分，但不引入 iterator/ownership。

| 当前入口与签名/行为 | 决策与目标签名 | 迁移边界 |
| --- | --- | --- |
| List/Map/Set/String `count/.count` 与 `Len/.len: C -> Number` 重叠 | **保留并首选** `.len`，收敛 Countable 的这些用途 | 参数单次求值；String 仍按标量。用户 Countable impl/bound 单独迁移；Struct 字段数与 Enum payload 数 **暂缓**，不把所有 count 当容器长度 |
| List `.add/.append: (List<T>, T) -> List<T>`；底层 Add trait 是两 List 组合 | **保留** `.append` 与 `.concat`，退场 List 单元素 `.add` 别名 | 先解决 originless `.add` 遮蔽；旧入口失效后也不能静默暴露另一种参数语义。名义 `Add` 调用/泛型界限保留单独验证 |
| Map `.assoc: (Map<K,V>, K,V) -> Map<K,V>` / `.dissoc: (Map<K,V>, K, & K) -> Map<K,V>`；Set `include/exclude` 返回新 Set | **本阶段首选**现有持久集合入口；不新增 Map/Set `.insert/.remove`，见 [#1479](https://github.com/calcit-lang/calcit/issues/1479) | Rust 同名方法可能暗示原地修改与不同返回值。Set `.add` 与 `.include` 同指 core `include`，可显式用 `core-set-include-v1` 迁移已证明的调用；Map `.add` 只验证二元 List entry 的形状，不能证明 key/value 为 K/V，查询状态为 `open`，结果类型降为 `Map<Dynamic,Dynamic>`。旧入口保留人工兼容，不自动改写为 `.assoc` |
| `foldl/.foldl/foldl'/reduce/.reduce: (List<T>, U, (U,T)->U) -> U` | **已引入** seeded `.fold`；`foldl'` 的内部备选实现与旧别名退场留给 [#1458](https://github.com/calcit-lang/calcit/issues/1458) | 保持左到右、空 List 返回初值、异类型 accumulator、callback 次数；不把旧 reduce 换成无初值语义。新方法证明不弱于现有 `.reduce` |
| `join/.join: (List<T>, T) -> List<T>` | **已引入** `intersperse/.intersperse`；旧名暂保留兼容 | 只插入同类型分隔项，保持空/单项/重复值/顺序；不改成 String 返回，也不与 `join-str` 混用 |
| `join-str/.join-str: (List<T>, String) -> String`，逐项格式化 | **已引入** `join-string/.join-string` | 保留 List<T>、原有显示规则和空 List 结果；旧名暂留兼容，不与返回 List 的 `intersperse` 混淆 |
| `vals` / Map `.values: Map<K,V> -> Set<V>`，去重 | **已引入** `distinct-values/.distinct-values`；旧名暂保留兼容 | 新旧都返回去重 Set，顺序不保证；保留重复值的视图是独立语义任务，本轮不复用旧名 |
| List `mapcat/.bind: (List<T>, (T)->List<U>) -> List<U>` | **已引入** `.flat-map` | 保持顺序、展平一层、callback 次数与具体 U；Fn `.bind` 是不同组合，**暂缓** |

Map/Set 更新均返回新集合，不修改原接收者。Map `.assoc key value` 替换已有 key，`.dissoc key` 忽略不存在的 key；Set `.include item` 去重，`.exclude item` 忽略不存在的元素，二者还接受更多同型元素。空集合遵循相同规则。Set `.add` 与 `.include` 是同一实现；Map `.add ([] key value)` 则先断言 entry 恰为二元 List，非法形状会失败，且无法从 `List<T>` 证明 key/value 分别符合 K/V。因此 Map `.add` 既不是 `.assoc` 的同签名别名，也不使 Map 实现 `Add` trait；`where T: Add` 的泛型调用会拒绝 Map，用户自定义的 `Add` impl 仍按其名义 trait 派发。

0.28.0 保留可精确查询的旧 Map `.add` 作为人工兼容入口，但不再作为 Agent 首选，也不提供自动改写。后续只有在 #1458 完成发布版匹配的真实消费者盘点与迁移、严格类型及 native/JS/受影响 WASM 回归、旧入口诊断和实现解耦后，才可在下一次 breaking release 移除；一次空的 fix 预览不构成移除证据。
| `.mappend` 在 List/Map/Set/String 上为各自组合，Fn 另有含义 | **已引入** Map `.merge`、Set `.union`；List `.concat` 已存在，但 List `.mappend` 的自动迁移 **暂缓**；String 与 Fn 另行审阅 | 分别验证重复 key 胜出方、去重；List 的包装函数与 `.concat` 参数契约不同，String 现有格式化宽度先核对，不跨 receiver 批量替换 |
| List/String `get/nth` 返回 Option；List `.find/.find-index` 接受 predicate，`.index-of` 接受元素；String `.find-index` 接受子串 | **保留**各命题，List 位置访问首选 `.get`；具体 List `.nth` 可用显式 `core-list-get-v1` 迁移 | 不把 predicate 查找改成值比较；缺失保持 Option，String 保持标量索引；Enum 异构位置访问 **暂缓** |
| `.map/.filter/.slice/.reverse/.sort/.keys`、`map-entries` | **保留**明确的现有词义 | `map-entries: Map<K,V> -> List<MapEntry<K,V>>` 保留 K/V；不为缩短名字退回异构 List<Dynamic> |

历史基线曾出现 `query type ":: 'List 'Number"` 的 `.reduce` 为 proven、`.append/.foldl` 为 open；目前这些方法已能从公开泛型 schema 获得 proven 契约。seeded `.fold` 也保持 `U, (U,T)->U -> U` 的 proven 方法契约；List `.reduce` 与 `.fold` 指向同一个 core 实现，可用显式 `core-list-fold-v1` 规则迁移已证明的调用。前缀 `reduce`、自定义 trait 和开放接收者不按名字批量改写，不能用 primitive 替换用户方法测试来绕过。

长度迁移的小批次使用显式 `core-collection-len-v1`：只在 List/Map/Set/String 的 `.count` 与 `.len` 同指对应 core 实现、方法契约已证明时改写。Struct/Enum 的 `.count` 另有语义，用户自定义 trait 也不据名字猜测；这不是一次全局文本替换。见 [fix 规则](../run/fix.md)。

List 分隔元素的小批次使用显式 `core-list-intersperse-v1`：仅当具体 List 上 `.join` 和 `.intersperse` 同指 `calcit.core/intersperse` 且契约已证明时改写方法调用。前缀 `join` 与返回 String 的 `join-str` 不在规则范围，旧名暂保留以便分批迁移。

List 文本拼接使用 `join-string` / `.join-string`，逐项沿用 `join-str` 的显示转换并插入 String 分隔符，空 List 返回空字符串；这不是返回 List 的 `intersperse`。显式 `core-list-join-string-v1` 只迁移具体 List 且旧新方法同指 `calcit.core/join-str`、类型契约一致的 `.join-str` 调用。前缀 `join-str` 需要独立审阅，不在此 fix 范围。

Map 去重值使用 `distinct-values` / `.distinct-values`，返回 `Set<V>`，不是保留重复值的 List。旧 `.values` 方法可用显式 `core-map-distinct-values-v1` 迁移：只有具体 Map 上旧、新方法均已证明指向同一实现且契约一致时才自动改写；前缀 `vals`、开放接收者与用户方法不按名字改写。

List 展平映射优先使用 `.flat-map`：回调对每个元素调用一次，返回 List 并按顺序展平一层，类型从 `List<T>` 与 `Fn(T)->List<U>` 推断为 `List<U>`。旧 List `.bind` 与新方法同指 `calcit.core/mapcat`，可用显式 `core-list-flat-map-v1` 在具体 List 且契约证明一致时迁移；前缀 `mapcat` 暂留，Fn `.bind` 不按名字替换。

集合组合的显式 `core-collection-combine-v1` 只迁移具体 Map `.mappend` → `.merge` 和具体 Set `.mappend` → `.union`，要求各组旧、新方法解析到同一个 core 实现且类型契约一致。Map 后面的相同 key 覆盖前值，Set 去重；List、String、Fn 的 `.mappend` 保持单独审阅，不做跨类型全局替换。旧名暂保留，等待消费者迁移验证后再考虑移除。

List 的位置读取优先用 `.get`：`List<T>` 的 `.get` 与兼容入口 `.nth` 均接收 Number 并返回 `Option<T>`，空表或越界返回 `none`。两者的方法查询现在能给出同一精确契约；`get` 前缀函数仍承担 Map、开放 Struct 等更广的查找语义。不要据名字把 Map/String/Enum 的 `get/nth` 批量互换，也不要把按 predicate 的 `.find/.find-index` 或按值的 `.index-of` 当作位置读取。

具体 List 的旧 `.nth index` 调用可显式使用 `core-list-get-v1` 迁移到 `.get index`：仅在旧、新方法同指 `calcit.core/get` 且 `Option<T>` 契约均已证明时给出自动改写；开放或自定义方法只供审阅。该规则不改 `:tests`、`:examples` 或 String/Enum 的索引调用，也不进入已发布 preset。

```cirru
do
  assert= ([] 1 0 2)
    ([] 1 2) .intersperse 0
  assert= |1-2 $
    [] 1 2
    , .join-string |-
  assert= (#{} 1)
    ({} (:a 1) (:b 1))
      , .distinct-values
  assert= 13 $
    [] 1 2
    , .fold 10 +
  assert= ([] 1 2 3)
    ([] 1 2) .append 3
```

### 转换、解析与名义构造

实施任务：[#1456](https://github.com/calcit-lang/calcit/issues/1456)。`!` 不表示“可能抛错”，`try-` 不表示异步，转换词不能充当类型断言。

| 当前契约 | 决策 | 类型/失败与迁移限制 |
| --- | --- | --- |
| `Option/Result/Struct` 名义构造；旧 `%some/%none/%ok/%err` | **保留**直接名义构造，旧 helper 属 **内部兼容** | 完整保留 payload/字段类型与错误；沿用现有 fix 与退场门禁 |
| `turn-str`、`turn-string` 为内部兼容函数，均转调 `to-string`，要求 `T: ToString`；`&turn-string` 仅是六类内建标量的底层实现 | **首选** `to-string: T where T: ToString -> String`；旧 `turn-*` 暂留兼容与宏阶段边界 | 六类标量和自定义名义 `ToString` 实现可通过兼容入口转换；List/Map/Set、Unit、FsPath 和未经解码的 Dynamic 在严格预处理时拒绝。nil 转空串，Tag/Symbol 取文本；已证明的 Number `ToString` 在 native/JS/core WASM/WASI 0.3 共用精确文本语义，包括运行时小数和非有限值。未证明的开放 trait/Dynamic 边界仍明确拒绝，不因 Number 支持而放宽；`to-string` 也不替代 Debug/Show。 |
| `turn-symbol` / `turn-tag` 的历史运行时输入较宽 | **已提供** `to-symbol: String -> Symbol`、`to-tag: String -> Tag`；旧入口暂留 core 宏与兼容路径 | 严格调用只接受 String；native/JS 已验证，WASM 尚无动态 Tag/Symbol intern，调用 `to-tag`/`turn-tag` 会显式报不支持而非伪装成 String。显式 `core-identity-conversion-v1` 只改写内建调用且参数被证明为 String 的稳定源码；也仅对已证明的六类内建标量迁移 `turn-string` 到 `to-string`，Dynamic/集合/自定义类型不自动改。旧入口兼容 Tag/Symbol 输入不构成新接口的静态类型承诺，不可全局替换 |
| `str`、`.debug/.show`、`format-cirru-edn`、`format-to-lisp/to-lispy-string` | **保留**显示/调试/序列化职责；`format-to-lisp` 与 `to-lispy-string` **不等价** | `format-to-lisp` 把代码/list 格式化成 Lisp 表达式，`to-lispy-string` 保留 Calcit 值构造表示：`([] 1 \|a)` 分别得到 `(1 \|a)` 与 `([] 1 \|a)`。二者都不是通用安全转换，不能自动互换；序列化默认 Cirru EDN |
| `.parse-json/.parse-cirru-edn/.parse-cirru/.parse-float` 返回 Result；`json-parse` 等旧入口抛错 | **保留并首选** `.parse-格式` 的 checked 路径；旧 throwing 入口属 **内部兼容** | `String -> Result<T,String>`；开放数据合法保留 Dynamic，typed parse-as 先证明 schema。`parse-float` 的 `NaN`、`inf`、`Infinity` 及带符号、大小写变体在 native/JS 返回 `:ok Number`，无效文本返回原文 `:err`；WASM 若将 `parse-float` 本身列为显式导出或入口 target，会在代码生成阶段因 `E_WASM_NIL_TYPE_EVIDENCE` 失败；若仅作为未支持依赖保留，生成的 stub 被调用时才陷阱，尚不能宣称支持该 Result 路径。throw→Result 是人工迁移，不能自动插入 unwrap/fallback；不新增公开 throwing 别名 |
| `number->int8` 等 `Number -> Result<Refinement,String>`；`js-nullish->option: JsNullish<T> -> Option<T>` | **保留**显式源→目标边界箭头 | 前者检查范围/整数/有限性，后者只包装、不验证 T；不并列再造 `to-/as-/into-` 同义入口，JS 后者仍受 `:js-ffi` 限制 |
| `strip-prefix/strip-suffix: (String,String) -> String`，未匹配保留原串 | **保留**现有签名和不匹配行为 | 不因为 Rust 同名 API 返回 Option 就夹带返回类型变更 |
| `fs:path: String -> FsPath` 与直接 Struct 构造 | **保留** `fs:path` 为应用首选；直接名义构造可追溯 | 两者创建相同 `FsPath :value String`，不访问文件系统、不规范化路径或检查存在；均在严格预处理时拒绝 Number。不新增 `to-path` 或无收益的构造器 fix；不要把内部 `fs-path:*` 当成应用首选 |

### 效果、宿主与内部实现

实施任务：[#1457](https://github.com/calcit-lang/calcit/issues/1457)；元数据与兼容入口收尾为 [#1458](https://github.com/calcit-lang/calcit/issues/1458)。统一 Calcit-facing adapter，不要求改动外部 JS 属性、native ABI 或 WIT 字段名。

`!` 的目标含义是**显式状态写入、外部动作、资源生命周期与回调注册/取消**。它不是纯度系统、错误标记或权限证明。普通文件/环境/时钟读取和随机数取值不因“不纯”自动加 `!`；消耗输入/等待另列有限例外。持久化集合的 insert/remove 返回新值，不加 `!`。

| 当前入口 | 决策与目标签名原则 | 边界 |
| --- | --- | --- |
| `reset!/swap!`；旧 `add-watch/remove-watch` | **已提供** `add-watch!/remove-watch!` 作为 Ref watcher 的首选注册/移除入口；旧名暂留底层兼容 | 两组调用共享原实现与 `Ref<T>`、`Tag`、`Unit` 契约；重复/缺失 key 仍报错，callback 次数不变；本阶段不自动改写未知同名调用 |
| FsPath 旧 `.write-text`；js-ffi `write-text!` | core **已提供** `.write-text!`；模块 **保留**已有 `!` | core 仍 `(FsPath,String)->Result<Unit,String>`，旧方法暂留兼容且共用实现；`core-effect-method-v1` 仅在类型与来源已证明时改写 core 调用；js-ffi 原有 Unit/throw/async 契约不因命名一致而自动统一 |
| FfiTask `.cancel/.cancel-with`，FfiResponse `.resolve/.reject` | **已提供** `.cancel!/.cancel-with!/.resolve!/.reject!`；旧名暂留兼容 | 新旧方法共用宿主实现，保持泛型、签名、exactly-once、释放与失败行为；`core-effect-method-v1` 可受控改写已证明调用，不能用返回 Bool 或命名代替生命周期证明 |
| `.read-text/.read-dir/.walk-dir`、`get-args/get-env`；std `read-file!/read-dir!/walk-dir!` | **保留**core 查询名字；std **目标**去掉读取的 `!` | 保留 Option/Result/throw 各自边界；模块分别 PR，不为命名增加新宿主能力 |
| `cpu-time: () -> Number` 实际为单调毫秒；`unix-time-ms` | **已提供** `monotonic-time-ms: () -> Number`；旧 `cpu-time` 暂留，`unix-time-ms` 保留 | 新入口复用旧时钟实现，只在同一运行内比较经过时间；native、JS、WASI Preview 1 可用，WASI 0.3 command 时钟仍明确不支持。std `get-time!/get-timestamp` 先核对返回模型再定映射 |
| 定时器注册/取消、`on-ctrl-c`；随机数、ID 生成 | **目标**注册/取消使用 `!`；随机/ID 的具体词汇 **暂缓** | 区分产生值与改变资源状态，核对 async、句柄和 callback；不按字符串后缀批量处理 |
| `println/echo/eprintln/read-stdin-text/wait-ms` | **保留**这些有限、按名明确的效果例外 | 输出、消费 stdin 和等待仍有真实效果；不是“所有 read-/print- 都自动例外” |
| `non-nil!` 是会失败的检查；旧 helper 中的 `!` | **暂缓**到 checked assertion/unwrap 命名明确 | 不改变失败模型，不将 `.unwrap` 自动插入业务代码 |
| `&trait::new/&impl::new` 对比 `&enum-def:new/&struct-def:new`，`is-spreading-mark?/data-definition-*/foldl'` | **内部**；先修角色元数据，后有界统一内部拼写 | 不给应用新增 alias；同步注册、macro、typing、lowering 与错误映射，不能为前缀整齐重做编译器 |
| `tuple?/tuple-enum` 迁移报错桩；Option/Result method helper | **内部兼容**，分别退场 | 报错桩与仍被方法引用的实现不是一类；`:internal` 也不能隐藏 Option、数学函数等实际公开能力 |

### 实施与验收顺序

1. 0.27.0：已合并的 Unicode 索引修复 → #1454 明确 nil/Option 与成员命题、修整数边界 → #1455 长度/List add 遮蔽 → seeded fold/intersperse → 去重 values 与其他组合。后续每批以对应 issue/PR 决定实际发布范围，不把整张表一次替换。
2. 0.28.0：#1456 转换/解析证据 → #1457 core/js-ffi/std 效果 adapter → #1458 已落地入口的 Agent 推荐、版本化 preset 和兼容清理；暂缓项不阻塞前一批，也不被视为默许改名。
3. 等价 fix 复用已有源码来源与 revision/fingerprint 防护；先 preview，再带 `--expect-revision` 应用，再次 preview 应为空。code、`:tests`、`:examples` 分别记录覆盖或人工处置；未知 macro、遮蔽、函数值、开放 receiver、自定义 trait 不猜。旧 preset 内容不变，新规则组合进后续版本的 preset，不新增顶层入口。
4. 每批保留 Calcit `:tests` 的用户方法调用，覆盖正常、空值、重复项、类型错误、失败与副作用顺序；Rust/脚本只验证 CLI、host/内存等边界。当前已支持的 native/JS/WASM/WASI 路径都验证，unsupported 明确列出，不能为测试绕过 lowering 改成 native call。
5. Respo 优先回归事件键、HTML/属性/样式输出与集合转换；js-ffi 回归 effect/JS 边界；std 核对时间/随机/文件模型。用实际 commit、entry、发布依赖记录证据，不写“所有消费者已迁移”。参考已核对的 Respo `respo.render.html/element->string` 中 `some?/turn-string/join-str`，旧 nil 判断不能自动换成 Option 判断。
6. 删除旧入口前同时满足：首选入口具有不弱于旧入口的类型证据；严格检查与相关 backend/真实消费者通过；fix 或人工迁移说明可用；至少一个正式版本的迁移窗口；core method 与待删应用入口解耦。到达版本号不自动授权删除；保留原因写到 issue，不让兼容名永久成为平行推荐。

本表来自 core metadata/method tables、`src/calcit/proc_name.rs`、native/JS builtin 实现、实时 `query type` 与 Respo/js-ffi/std 用法核对。它不是新的运行时 source of truth；实际 API 仍由 schema、解析到的定义和测试决定。每阶段完成后独立发版并发布中文成果 Discussion，不把本规划当作所有功能已交付。

## 可运行的角色示例

`Option` 是名义类型定义；加上 variant 后，同一定义也能直接构造值。类型表达式仍写在 schema 中，不能把 `Option` 裸名当成已构造的值：

```cirru
assert= 3 $ .unwrap $ Option :some 3
```

方法在具体接收者上保留类型关系，失败行为也属于该方法的契约：

```cirru
assert= 3 $
  Option :some 3
  , .unwrap
```

无接收者的解析操作保持命名空间函数形式，其返回值是 Result：

```cirru
assert= (Result :ok 3) (parse-float |3)
```

String 的 `.includes?` 检查子串；`.contains-index?` 在 String 上检查**字符索引**是否有效，不能只凭英文词形把两者当同义词。List 和 Map 的 `.get` 都返回 Option：

```cirru
do
  assert= true $ |abc .includes? |b
  assert= true $ |abc .contains-index? 1
  assert= false $ |abc .contains-index? 3
  assert= (Option :some 2)
    ([] 1 2 3) .get 1
  assert= (Option :some 2)
    ({} (:x 2))
      , .get :x
```

下面是内部 primitive 的定位示例，不是应用代码的推荐写法；它和上面的 `.includes?` 在这个输入上结果相同，但应用应先使用方法：

```cirru
assert= true $ &str:includes? |abc |b
```

## Agent 查询顺序

### 核心 API 稳定与集中迁移

这是 [#1568](https://github.com/calcit-lang/calcit/issues/1568) 的策略基础。**0.28.0 的冻结清单与 CI 门禁尚在实施，不代表本页所有目标名都已经发布或冻结。** 上文的“目标”“暂缓”和未通过消费者验收的条目不能成为稳定 API 承诺。使用者先查询已安装版本，维护者再依据发布版证据完成清单。

冻结的是用户层的调用契约，不是编译器内部表示。审阅后的清单需要覆盖首选类型/构造入口、普通函数、trait 与接收者方法；每项记录公开名字、接收者与参数顺序、optional/rest、泛型和 `where` 约束、返回类型、失败语义及实际 backend 支持范围。`Option` 等名义类型不能因带 `:internal` 而漏掉；方法的 `definition` 只是实现路径，`&` primitive、宏展开 helper 与 lowering 名不因此冻结。core 之外的模块由各自仓库维护版本契约，不用 core 清单冻结 npm、native ABI 或 WIT 的外部名字。

清单应从已有 `query def/context/type --format edn` 的结构化证据整理，而不是新增公开 API registry 或查询命令。函数 schema 与具体 receiver 上的方法契约分别导出：只抓一个函数定义无法证明方法派发、歧义、generic bound 或所有 backend 支持。保留原生 Cirru EDN 的 tag、symbol 与类型表达，不把展示字符串作为签名的唯一证据。清单是发布与 review 的基线，不是另一份驱动运行时的定义。

冻结之后，普通版本可以新增不冲突的能力和修复实现错误，但不改已有首选名字、参数/返回类型或合法输入的契约，不用同名 API 悄悄切换命题。修复跨 backend 不一致时仍须说明可观察变化并验证共享 Calcit `:tests`；不能以“bugfix”为由绕过真实消费者检查。确需改名或 breaking contract 的工作集中到明确标记的迁移版本：同一 PR 提供旧新映射、类型与失败差异、升级指南及可验证的消费者证据。

这不冻结类型推导算法、macro 展开或 backend lowering 的内部实现，也不要求保留不必要的 Dynamic。更精确地推断已有结果、提前拒绝原契约之外的输入，应保留合法调用语义并补严格正/负例；若要收窄原本合法的动态输入或改成功/失败模型，则是明确的契约迁移，不能通过修改“公开”的分类绕过。冻结清单中的 `open/ambiguous` 证据不能伪装成 proven，更不能把宽 schema 的类型漏洞变成长期稳定承诺。

迁移先判断是不是**同一调用契约**。来源可唯一定位、参数/返回与失败行为相同、求值次数/次序不变时，优先用确定性映射交给现有 `fix`；只有映射不足以证明方法/类型关系时才增加证明条件。throw → Result、String → Option、去重 → 保留重复、单位改变或泛型约束收紧不是纯改名：需要人工迁移说明，不自动插入 unwrap、fallback 或 unsafe，也不能通过改名表把任意 signature 变化洗成等价。旧 preset 内容不变，集中迁移使用新的版本化组合。

每种任务只推荐一个首选写法。兼容名可以被精确查询，以便旧代码找得到迁移路径，但不能与首选名并列生成新代码。别名记录必须给出首选名、同契约与否、现有 fix/人工步骤、首次提供的正式版本、计划移除的集中版本，以及当前阻塞证据。尚未决定移除版本的条目必须标为未完成排期，不能把“以后”“下一次 breaking”当作已经完成 #1568 的移除版本验收。到达计划版本仍须满足 #1451 的发布窗口、真实消费者迁移、严格类型/运行验证与 core 实现解耦条件；内部 helper 的应用兼容入口和方法仍需要的实现目标分别处理。

冻结门禁的验收应覆盖两类失败：删除/改名，以及 schema/receiver/generic/失败契约变化。CI 必须能用受控负例证明未经迁移的变化被拒绝，同时验证合法新增与内部实现重构不被误阻断。显式迁移版本的例外需要同一 PR 的映射和相应证明；只更新 baseline 文件或文档不能自动放行。失败语义与 backend 覆盖仍由 Calcit `:tests`、严格负例及既有 host 测试共同证明，不另建统计 analyzer。

在清单、别名排期与上述门禁完成前，#1568 和关联收尾 issues 保持开放。core 可先发布经过验证的版本，再以该正式版或明确标记的 alpha 验证模块消费者；milestone 只有在消费者和收尾验收完成、正式发版及中文成果 Discussion 发布后才标记完成，避免“要先关闭 milestone 才能发布、又要先发布才能迁移”的循环。

先查类型再选方法，不要从模糊搜索到的内部定义名猜调用形式：

```bash
calcit docs read api-roles.md '逐族命名决策'
calcit docs read api-roles.md '谓词与成员查询'
calcit query type ":: 'Option 'Number"
calcit query type "'String"
calcit query type ":: 'List 'Number"
calcit query type ":: 'Map 'Tag 'Number"
calcit query def 'calcit.core/Option' --format edn
calcit query def 'calcit.core/%some' --format edn
calcit query context 'calcit.core/option:unwrap' --format edn
```

`docs read` 的默认 guidebook 来自已安装的 `~/.config/calcit/docs`，不是当前工作目录的源码，也不会因重编译 CLI 自动更新。找不到新章节时，先用 `docs sections api-roles.md` 核对已安装文档版本，再按已有文档安装流程更新；不要为查新名字再创建查询入口。开发中的 Markdown 可直接用 `docs check-md <path> --snapshot <snapshot>` 验证，模块文档仍用现有 `--module` 参数查询。

`query type` 的 `proven` 表示当前类型能证明该方法的调用契约；`open` 或 `ambiguous` 不是类型安全的肯定结论。对已由显式 fix 证明为同一实现/签名的别名，查询另给 `preferred` 或 `compatibility` 角色，兼容项可追溯首选名和 fix 规则。human 输出按已证明的首选入口、尚未分类的方法、兼容入口分组；未标角色的方法不应被推断为过期，`open` 的方法也不因角色标为首选就变成已证明。方法旁边的 definition path 用于追踪实现。再用 `query def/context` 读取 `:constructor` 或 `:internal` 标签、schema 与示例；需要机器处理时优先用 `--format edn`。项目代码的类型证据可用 `query type-at` 或 `query context` 查看。

结构升级后，使用 `calcit fix --preset core-api-0.28-v1 --format edn` 统一预览已证明的核心 API 叶子改名，再按 revision 应用。`:code` 自动扫描，attached `:tests` / `:examples` 人工核对；重复预览的自动建议应为空，剩余 `requires-review` 不等于已迁移。Option/Result helper 和完整构造调用仍分步迁移，顺序与范围见 [0.28 核心 API 命名迁移](../run/fix.md#028-核心-api-命名迁移)。

真实项目中，Quamolit 的 `quamolit.gpu-scalar-program/first-slot` 以 `get slots 0` 得到 `Option<BoundScalar>`，再调用 `.unwrap`；Timegrass 的 `app.server/main!` 对 `parse-float raw` 的 `Result<Number,String>` 调用 `.unwrap-or 11009`。这两条路径均可在各自 Snapshot 上用 `query context` 找到，再用 `query type ":: 'Option ..."` 或 `query type ":: 'Result ..."` 检查方法契约。它们说明推荐入口应由返回类型决定，而不是由内部函数名字决定；不要求改变这些项目的源码。

`%some/%none/%ok/%err` 从 0.25.0 起标记弃用，但不能仅凭最早可移除的版本号删除。先用 `calcit fix --rule core-nominal-constructor-v1 --format edn` 预览，再带预览返回的 `--expect-revision` 应用；真实消费者、发布版依赖、严格检查、相关 backend 与实现解耦的退场条件仍按 [旧方法 helper 的退场条件](#旧方法-helper-的退场条件) 和 #1458 逐项核对。规则只自动改写编译器已解析到 core、实参数量匹配且处在可证明保序的源码调用；把 helper 当函数值、局部遮蔽或跨未证明的 macro 边界时需人工确认，不会插入 Dynamic 或 `unsafe-coerce`。String 旧 `.contains?` 检查索引而 `.includes?` 检查子串；已证明的具体 String 调用可用 `core-predicate-method-v1` 显式迁往 `.contains-index?`，不能在跨容器调用上按词形全局替换。

对 Option 的旧 helper，可用 `calcit fix --rule core-option-method-v1 --format edn` 预览 `.some?` / `.none?` / `.unwrap` / `.unwrap-or` 迁移。只有接收者方法解析为 `proven` 且实现确实指向相同 core helper 时才提供自动改写；已核实单次保留调用的 core `let`、`cond`、`do`、`fn`、`assert=` 宏允许通过，其他宏仍需审阅。已证明来自 core 的空值 `%none` 或 `Option :none` 即使起初显示 `Option<Dynamic>`，也可由具体 fallback 推出 payload 类型，再检查方法分派；普通开放接收者、函数值或无来源映射的表达式仍不能自动改写。应用后重复预览应为空，并运行严格类型检查与项目测试；不把 String 或其他内部函数类推为同一个迁移。

Result 的 `result:ok?` / `result:err?` / `result:unwrap-or` 沿用单独的 `core-result-method-v1` 规则，不新增顶层 CLI 入口。只有编译器能证明同一方法契约才自动迁移；已证明来自 core 的 `%err` / `Result :err` 不携带成功值，因此允许具体 fallback 绑定成功类型，同时仍检查错误类型。成功值已有具体类型的 core `%ok` 可按同一方法契约迁移；普通开放 Result 和 `:ok` 的动态成功值不能凭 fallback 收窄，具名构造缺少精确源码证据时也保留审阅。旧 helper 作为 core 实现暂留；公开文档只推荐 `.ok?` / `.err?` / `.unwrap-or`。在真实消费者完成迁移、Agent 查询和升级文档统一首选写法、经过至少一个发布窗口且 JS/native/WASI 相关测试通过之前，不删除旧 helper。其他 `result:*` 逐项确认语义与迁移证据，不跟随批量废弃。

### 旧方法 helper 的退场条件

0.26.0 只收敛**应用源码的首选入口**，不直接删除 `option:some?`、`option:none?`、`option:unwrap`、`option:unwrap-or`、`result:ok?`、`result:err?`、`result:unwrap-or`。这些名字目前也是 core trait method 的实现目标；直接删除会破坏推荐的 `.method`，而不是仅移除旧别名。旧 helper 暂作内部兼容实现，不把它们和方法并列推荐，也不把内部调用数量当作应用迁移进度。

最早在 0.27.0 考虑移除其**应用可直接调用**的兼容入口，且须同时满足：真实消费者在发布版 Calcit 与匹配的运行时依赖上完成严格类型和运行测试；对消费者 Snapshot 重复运行对应 fix 无可自动改写的旧调用，剩余 `requires-review` 已逐一处置；Agent 查询、升级文档与示例只推荐方法；至少经历一个已发布版本的迁移窗口；JS/native 及实际受影响的 WASM/WASI 路径验证通过；core method 实现已与将删除的入口解耦，并有 Calcit `:tests` 证明行为不变。任一条件未满足就继续保留兼容入口，记录原因和下一次检查的版本，不通过扩大 Dynamic 或机械替换绕过。此约束不适用于无自然接收者的模块函数，也不承诺把所有 `result:*` / `option:*` 内部实现一并移除。
