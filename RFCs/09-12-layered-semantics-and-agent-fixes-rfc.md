# RFC：Calcit 分层语义与 Agent 可执行修复契约

状态：Accepted — 0.14.13

日期：2026-09-12

关联：#997、#998、#999、#989、#992–#996

## 1. 目标

Calcit 的语言语义面向人，程序存储和工具协议同时面向人和机器。两者不要求只有一层实现：

```text
Cirru/Snapshot 结构化程序
  → 人编写和审查的表层语言
  → macro 展开、名称解析和类型分析后的 core language
  → native / JS / WASM backend
  → 显式 host/FFI 边界
```

分层的目的不是增加多套规则，而是让每条语义只有一个 owner。表层语言定义用户看到的行为；core
language 保存已经证明的关系；backend 实现共同语义或明确拒绝尚未支持的子集；host boundary 验证
编译器无法从外部实现中证明的事实。

本 RFC 是 #998 结构化 fix 协议和 #999 自动迁移的语义前提，不新增第二套类型系统、平行 runtime
IR 或新的质量统计体系。

## 2. 结构化程序层

`calcit.cirru` 是 Cirru EDN Snapshot，也是 definition、schema、examples、tests 和配置的结构化事实来源。
它不是生成 JS、pretty-print 文本或 macro 展开树的序列化副本。

这一层拥有：

- 稳定的 definition identity；
- definition 内部的 quoted source AST；
- schema、examples、tests 与文档；
- Snapshot revision 和可定位 subtree 的 fingerprint；
- 通过 `calcit query` / `calcit edit` / `calcit tree` 暴露的事务读写。

数字 path 只在某个 revision 内有效。Agent 或迁移工具写入前必须同时验证 revision，必要时验证 subtree
fingerprint；冲突时重新查询，不能根据旧坐标猜测新目标。

## 3. 表层语言

表层语言是兼容性和文档的主要对象。它包括用户直接使用的 syntax、macro、函数、method、nominal
Struct/Enum、trait、Option/Result、集合和显式 FFI adapter。

表层语言拥有：

- 求值顺序、副作用次数和错误行为；
- 公共函数与 method 的参数、返回值和失败契约；
- 哪些位置允许开放数据，哪些位置需要具体类型；
- macro 对调用者承诺的语义，而不是某次实现生成的内部树；
- 可由用户直接阅读的 examples 与 definition `:tests`。

编译器优化、backend 限制或实现方便不能静默改变这些行为。需要改变表层契约时，应先提供版本化迁移和
可审查的 source fix；仅有 lowering 能处理新形式，不等于迁移已经完成。

## 4. 展开后的 typed core

core language 是 macro 展开、名称解析、控制流分析和必要的类型导向归约之后，由 compiler 拥有的
语义表示。它可以包含用户不会手写的 primitive、resolved definition、字段索引或静态 method target。

这一层拥有：

- 已解析的 definition/type/method identity；
- 局部推断和公共 schema 共同建立的类型关系；
- 明确的求值顺序、分支和调用关系；
- backend eligibility 所需的能力证据；
- 每个生成节点回到表层 source AST 的 origin chain。

core language 不拥有另一套用户类型规则。类型导向 lowering 必须保持表层求值次数、顺序、返回值与失败
行为。无法证明等价时保留原表达或给出诊断，不以 Dynamic fallback 伪装成已成功 specialization。

compiler-owned lowering 不直接写回 Snapshot。即使某段展开树是正确的，也可能丢失 macro 所表达的人类
意图；只有能唯一回到表层节点、并证明表层语义等价的建议才能成为 source fix。

## 5. backend 与 host boundary

native 和 JS 是公开语义 backend。它们对共同支持的表层程序必须产生一致的可观察结果；生成成功只证明
lowering 完成，不证明生成模块可以加载或程序行为正确，因此共同语义需要实际执行验证。

WASM 是优先扩展的编译 backend，可以阶段性只支持 typed core 的明确子集。它必须：

- 在 lowering 前验证整个可达单元；
- 对未支持的 syntax/type/capability 给出结构化诊断；
- 保留原始 definition、source path 和 origin chain；
- 不静默转回 native/JS，也不发展独立的表层类型政策；
- 以共享 Calcit 语义用例逐步扩大覆盖，而不是为快速通过 lowering 引入 backend 专属语言规则。

JS/native FFI、网络数据和硬件输入属于 host boundary。外部声明提供静态契约，但编译器无法证明声明与
宿主实现或实际数据一致；adapter 必须通过 decode/validation 将外部事实转换成普通 Calcit nominal
data、Option 或 Result。backend bug、外部数据错误和应用业务错误应分别验证，不能都归为用户类型问题。

## 6. Unknown、Dynamic 与宿主对象

这三个概念承担不同职责：

### 6.1 Unknown/Unresolved

Unknown/Unresolved 是编译器尚未建立充分证据的内部状态，例如未绑定 TypeVar、缺少上下文的空集合或未解析
type slot。它不是用户 schema，不进入普通 runtime value，也不能像 Dynamic 一样匹配任意类型。

编译器应继续利用 expected type、调用参数和控制流求解它；严格模式最终仍无法求解时给出稳定错误。兼容模式
若必须降为 Dynamic，应同时保留 evidence-loss 诊断，不能让该 fallback 反过来影响严格语义。

### 6.2 Dynamic

Dynamic 是用户明确选择的开放 Calcit 值。它允许内容暂时未知，但不意味着内容可以被当成任意具体类型。

允许的基本关系是：

- 保存、返回或跨函数传递 Dynamic；
- 放入明确声明的开放 Map/List；
- 包装和传递 `Option<Dynamic>` / `Result<Dynamic, E>`；
- 参与不观察其具体内容、且契约明确的结构操作。

将 Dynamic 当作 Number/String、访问具体 Struct 字段、调用需要特定 trait 的能力，必须先 decode、narrow，
或进入显式 unsafe boundary。泛型函数若真正对 `'T` 参数化，可以用 `Dynamic` 实例化 `'T`；若函数通过
`'T` 声称了内容相等、具体 trait 或其他无法证明的关系，则继续拒绝。

因此 #994–#996 的目标是修复“安全携带未知 payload”与“具体使用 payload”之间的界线，而不是复制一套
`*-dynamic` 标准库。

### 6.3 JsObject 与其他 host value

`JsObject` 表示宿主拥有、Calcit 不知道内部形状的值。它与普通 Dynamic 分离：Dynamic 仍是 Calcit runtime
value，JsObject 的属性、方法、生命周期和空值行为受 JS 边界契约控制。其他 native/hardware resource 也应
采用同样的 opaque handle + typed adapter 思路，不把宿主事实伪装成普通类型推导结果。

## 7. 可读的共同语义例子

每项新语义先用表层 Calcit 例子说明，再决定 core lowering 与 backend 实现。例子至少覆盖三类结果：接受并
执行、编译期拒绝、当前 backend 明确不支持。

开放值读取的目标契约示例：

```cirru.no-check
let
    data $ assert-type ({} (:count 1) (:title |demo)) (:: 'Map 'Tag 'Dynamic)
    value $ .get data :count
  ; Transport value as Option<Dynamic>; narrow/decode before Number operations.
  consume-open-option value
```

具体类型边界示例：

```cirru.no-check
defn parse-device-message (raw)
  ; The adapter returns Result<DeviceMessage, DecodeError>.
  decode-device-message raw
```

这些示例在相关能力落地前使用 `cirru.no-check`，不能把文档片段可解析冒充为实现已经通过。实现 PR 应把
对应行为移入 definition `:tests` 或专用成功/失败 fixture，并精确记录实际执行的 backend。

### 7.1 已落地的代表性证据

下表把分层契约映射到仓库中已经执行的 Calcit 程序。这里记录的是可复用的语义证据，不引入覆盖率、数量目标
或新的 analyzer。Snapshot 中的 definition 是契约主体；脚本只负责选择 backend、执行产物和防止零测试误报。

| 语义切片 | 表层 Calcit 证据 | backend 证据 | 分层结论 |
|---|---|---|---|
| 普通 typed value 与泛型推导 | `calcit/test-types-inference.cirru` 的 `test-generics-identity`、`test-map-inference` 与 `test-struct-inference` | `yarn try-rs` 与 `yarn try-js` 都从 `calcit/test.cirru` 执行这些断言 | 类型关系属于 surface/typed core；JS lowering 不另定义泛型规则 |
| 开放 Map/List 与 `Option<Dynamic>` | `calcit/test-traits.cirru` 的开放集合读取断言，以及 `calcit.core/get` 的 `reads-open-map-payload` definition test | native/JS 主测试执行前者；`yarn try-core-tests` 用 `--require-match` 执行后者 | 未知 payload 可被保存、读取和包装；具体使用仍需 narrow/decode |
| macro 展开与求值语义 | `calcit/test-macro.cirru` 的 `test-destruct`、`test-lambda` 等 Calcit 断言 | native/JS 主测试执行同一表层 macro 调用；展开比较只作为 core 形状证据 | surface macro 拥有行为，展开后的 core 必须保持求值与词法作用域 |
| JS FFI host boundary | `calcit/test-js.cirru` 的 `test-property`、`test-collection` 和显式 `unsafe-coerce` 用例 | `yarn try-js` 生成 JS 后由 Node.js 实际执行；native 不伪装支持 JS host value | `JsObject` 是显式 backend/host 能力，不以 Dynamic 推导替代 adapter 证据 |
| 当前 WASM typed 子集 | `calcit/test-wasm.cirru` 的 `test-static-option-result-methods` 等带 `:wasm` tag 的 definition tests | `scripts/test-wasm.sh` 先以 `--require-match` 执行 Calcit tests，再加载生成模块并检查 fail-closed 边界 | WASM 复用 typed core 语义；支持项实际执行，未支持项拒绝产物而非返回占位值 |

这组证据也明确了“不共同支持”的处理方式：JS FFI 不要求 native/WASM 模拟宿主对象，WASM 尚未覆盖的语法也
不要求表层退化。backend 只实现已声明能力，差异在 capability boundary 或稳定 unsupported diagnostic 中显式出现。

## 8. Agent 语义协议

Agent 不应从 human diagnostics、生成 JS 或格式化文本反推语义。query、diagnostic、expansion trace 和 fix
suggestion 应共享 versioned typed result，并至少能表达：

```cirru
{}
  :schema-version 1
  :command :analyze.semantic
  :revision |opaque-snapshot-hash
  :data $ {}
    :definition 'app.core/main!
    :semantic-layer :surface
    :path |code@3.2
    :fingerprint |opaque-subtree-hash
    :diagnostic-code |E_EXAMPLE
    :origin-chain $ []
    :replacement nil
  :diagnostics $ []
  :next $ []
```

协议规则：

- EDN 是 Calcit 值的权威机器表示，JSON 是稳定兼容投影；两者来自同一个 typed result；
- stdout 只包含一个完整机器值，日志、进度和提示进入 stderr；
- `:semantic-layer` 至少能区分 `:surface`、`:core`、`:backend`、`:host-boundary`；
- `:origin-chain` 从当前节点指向 macro 调用和原始 source 节点；
- 无唯一 source origin 或无法证明等价的建议，`:replacement` 必须为 nil；
- 可应用 replacement 使用 quoted AST 和 revision/fingerprint 前置条件，不使用行号 patch；
- 相同 revision、scope 和 rule 的结果必须确定且可重复。

这些字段是能力契约，不要求所有命令在 #997 一次实现完成。#998 负责落地最小 fix suggestion 与原子应用闭环。

## 9. Source fix 与积极迁移

Calcit 生态规模仍小，且维护中的消费者多数在 JS 路径，因此 0.15.0 可以采用较积极的表层收敛，但必须满足：

1. breaking change 的 fix 先由已发布的 0.14.x 工具链提供；
2. fix 在 Snapshot source AST 上工作，不编辑生成 JS；
3. 同一规则重复运行幂等，stale revision 或歧义不写入；
4. 先在 staged Snapshot 完成 parse/schema/preprocess，再执行相关 Calcit、native、JS 和项目构建验证；
5. 自动 PR 同时展示人可读 semantic diff 和 stable rule ID；
6. 含业务默认值、unsafe、错误处理选择或求值变化的建议只报告，不自动应用；
7. 生态迁移完成后删除被取代的 helper、compiler special case 和旧文档，避免永久维护两种表层写法。

“可自动应用”和“需要 review”是操作结论，不发展成新的质量评分或统计目标。迁移成功以行为一致、差异可读和
旧规则能够删除为准。

## 10. 验证矩阵

| 契约 | 主要验证 | 不能单独作为证明 |
|---|---|---|
| 表层类型与行为 | definition `:tests`、成功/失败 fixture | Rust helper 单测 |
| macro 语义 | 表层调用 + 展开 origin fixture | 某一次 pretty-print 展开文本 |
| typed core lowering | 等价前后行为、求值顺序、source mapping | 仅比较内部 enum variant |
| native/JS 共同语义 | 同一 Calcit case 的实际执行 | check-only、JS emit 成功 |
| WASM 子集 | 共享 Calcit case + 实际执行或明确 unsupported | emit 成功、清单分类本身 |
| host/FFI | adapter contract、错误路径、真实或 faithful fake host | 类型声明本身 |
| source fix | before/after/must-not-rewrite、幂等和 post-check | suggestion 数量或应用率 |

Rust 测试继续承担 parser/serializer、内部数据结构、原子写入、冲突和 backend 低层 invariant。用户可观察语义
默认通过 Calcit 表达；测试选择为零不能报告成功。

## 11. 与既有路线的关系

- `08-21-static-type-system-evolution-roadmap.md` 继续描述类型能力顺序；本 RFC 补充分层 owner 和开放值契约。
- `07-26-agent-machine-protocol-rfc.md` 继续拥有 typed result/EDN/JSON envelope；本 RFC 增加 semantic layer、
  origin chain 和 source fix 的约束。
- `07-26-safe-structured-editing-rfc.md` 继续拥有 revision、fingerprint、transaction 和 atomic write。
- `04-13-call-arg-literal-rewrite-rfc.md` 与类型导向优化继续属于 compiler-owned lowering，不自动成为 source fix。
- WASM lowering 继续限定 backend 支持子集，并复用这里的表层身份、类型证明与诊断规则；后续按共享语义用例扩大覆盖。

后续新增语言规则应先回答它属于哪一层、替代了什么旧规则、用户需要理解什么，以及能否删除已有特例。若只能
通过增加新 analyzer、平行 API 或 backend-specific 表层政策解释，默认回到共同语义重新设计。
