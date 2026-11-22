---
title: "WASM 支持矩阵"
summary: "calcit wasm / calcit wasi 能编译哪些构造、需要什么类型证据，以及每个 E_WASM_* 诊断的含义"
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "wasm support matrix"
  - "wasm diagnostics"
  - "E_WASM"
  - "wasm unsupported"
---
# WASM 支持矩阵

本页说明一段 Calcit 代码能否由 `calcit wasm`（browser/embedded core module）与 `calcit wasi`（WASI `command` world，或通过 `--world http` 面向 `wasi:http/proxy@0.2.12`）编译。共享的纯语言降低与类型/能力校验在各目标间复用，下表中的 WASI 宿主能力行默认描述 `command` world（`wasi:cli/command@0.3.1`）；`--world http` 复用纯语言降低与 `println` / `eprintln` / `echo` 标准日志流，并拒绝仅限 `command` 的宿主能力（`get-args`、`get-env`、`read-stdin-text`、`FsPath`、时钟、`wait-ms`、`secure-random-bytes`、`quit!`），用法与限制见 [WASI HTTP proxy (`--world http`)](../run/cli-options.md#wasi-http-proxy---world-http)。WASI 宿主能力另见 [宿主能力边界](host-capability-boundary.md)，Component 类型见 [WASM Component 边界](wasm-component-boundary.md)。

## 状态

- **supported**：直接编译，结果与 native 一致，由 core `:tests` 在 WASM 上回放验证。
- **supported-with-type-evidence**：只有预处理已经证明了闭合的具体类型时才能编译，缺少证据时报告对应的 `E_WASM_*` 诊断。补充证据的方式是给参数、返回值或局部绑定写出具体类型，或改用已证明的 nominal 构造，而不是把类型放宽为 Dynamic。
- **unsupported**：当前 WASM 后端明确拒绝，诊断为 `unsupported ... in WASM` 或 `E_WASM_CAPABILITY` 等；这是目标后端的支持边界，不改变该构造在 native 与 JS 上的语义。

普通依赖里不受支持的定义会编译为 trap 桩，只有调用到时才 trap；显式导出与 command 入口在编译期报告。生成了产物不代表可用，应实际调用入口验证。

## 表层构造

| 构造 | 状态 | 证据或说明 | 验证 |
| --- | --- | --- | --- |
| `if`、`when`、`when-not`、`if-not`、`and`、`or`、`not` 与 `defn` / `let` / `recur` 尾递归 | supported | 条件统一严格要求静态与运行时 `Bool`（非 `Bool` 静态报 `E_NON_BOOL_CONDITION` / `E_PROC_ARG_TYPE_MISMATCH`）；已证明封闭标量与非空堆指针在内部直调中使用原生 `i32`/`i64` 槽位，原生宿主导出、间接调用表与开放类型保留 f64 值 ABI | core `:tests`，如 `calcit.core/recur`、`calcit.core/if` |
| Number、Bool、nil、Unit、String、tag 字面量 | supported | 仅在开放/擦除类型或 `=` 比较等通用值 ABI 下 nil、false、Unit 与 0 共用 f64 表示，见下文诊断说明；条件分支已限定为 `Bool` | core `:tests` |
| 具体标量输入的 `type-of`、`number?`、`bool?` | supported | 已证明的 Number（含数值 refinement）、Bool、Nil、Unit 优先使用真实类型证据，实参仍求值一次；不会将 false／nil／Unit 的零值重新解释为 Number，合法 Number 也不会被同值堆地址覆盖 | `calcit.core/number?` 的 `:scalar-kind-evidence` 测试与 WASM 参数失败路径回归 |
| `nil?`、`nil->option` 与 Option 在可空拼接／开放类型上的判断 | supported-with-type-evidence | 需要具体类型区分 nil、false 与 0（`E_WASM_NIL_TYPE_EVIDENCE`）；直连 nil 拼接的 `nil->option`、字面量 `get-in` / `dissoc-in` / `assoc-in` / `update-in` 已按闭合静态路径降低，其余开放遍历仍按 `E_WASM_CALL_SPECIALIZATION` 等诊断拒绝 | `calcit.core/nil->option#collapses-nil-joins-under-declared-option`、`calcit.core/get-in#handles-empty-path-and-nil-base`、`calcit.core/dissoc-in#dissocs-nested-maps-and-list-indexes` |
| 数值排序比较 `&<`、`&>` 及其表层调用 | supported-with-type-evidence | 操作数需要 Number 证据；已知非 Number 保留实参求值后 trap，开放类型在 scalar 擦除前报告 `E_WASM_NIL_TYPE_EVIDENCE`。具体 Number 的泛型调用复用调用点特化；`identical?` 不受数值限制 | `calcit.core/contains?` 的 `:enum-numeric-evidence` 测试与 WASM 宿主失败路径回归 |
| List / Map / Set / Struct / Enum 值与 core 方法 | supported | 线性内存 + bump allocator；`#{}` 字面量与 `&list:to-set` 按静态元素类型去重 | core `:tests` |
| `foldl` / `&list:foldl` | supported | List 和 Set 逐项传给回调。Map 逐项传入二元 List `[key, value]`，静态已知的 Map 为回调保留 `Tuple K V` 类型，可用 `let[]` 分别取得键和值，也可显式接收 `List Dynamic`。空集合返回初始值；开放接收者按实际容器分派，其他值会 trap | `calcit.core/foldl#folds-map-as-key-value-lists`、`calcit.core/foldl#keeps-map-key-and-value-types`、`calcit.core/&list:foldl#keeps-map-key-and-value-types` |
| `update` 作用于 List / Map / Enum | supported | 按运行时类型分派：List 下标越界或 Map key 不存在时原样返回且不调用回调；Enum 下标越界时 trap，对应 native 的 range error | `calcit.core/update#updates-list-by-index-and-skips-missing-slots` |
| 已证明的普通方法调用、trait 方法 | supported-with-type-evidence | 普通方法复用接收者的实际实现：List、Map、Set 等内建类型使用固定方法表，成员类型仍可为泛型；名义值还需证明实际附带的实现身份。显式 trait 调用仍需具体类型（`E_WASM_TRAIT_TYPE_EVIDENCE`、`E_WASM_TRAIT_RESOLUTION`） | `test-types-inference.main/find-last-positive-index#typed-number-receiver`、`find-last-matching-index#keeps-number-and-string-element-types`；名义实现由 `:impl-origin-portable` 测试覆盖 |
| `slice`、`.slice` 与自定义 `Sliceable` | supported-with-type-evidence | 终点为 `Option<Number>`：`Option :some end` 指定终点，省略或 `Option :none` 取到末尾；接收者和实现需要可静态解析，保留 String 的 Unicode 标量单位与 List 成员类型 | `calcit.core/slice#option-endpoints-through-function-method-and-trait`、`test-types-inference.main/slice-with-contract` 的四项共享测试 |
| 固定参数函数、普通内联闭包与立即调用的尾部 Option 缺省 | supported-with-type-evidence | 省略实参补入真实名义 `Option :none`，不使用 nil；已知不可变泛型函数的局部别名在调用点取得参数证据后特化 | `calcit.core/slice#inline-option-omission-keeps-nominal-none`、`test-types-inference.main/slice-with-contract#custom-function-reference`、`test-types-inference.main/option-recur-sum#immediate-option-completion` |
| 已知目标的函数值、局部别名与函数组合 | supported-with-type-evidence | 目标明确时复用具名调用的 lowering，支持别名链、词法遮蔽、闭包捕获、`map/foldl`、具名 rest / 字面量 spread；`Fn` 的 `.map` / `.mappend` / `.bind` / `.apply` 组合及 tag key 的 `.sort-by` 在调用点静态特化；内联调用的回调实参若为明确的具名定义，也保留目标。函数值可保存并保持身份，实参按原顺序各求值一次；WASI 的未知间接目标与不支持的可达宿主能力仍拒绝 | `app.wasm-alias/bump`、`app.wasm-alias/sum-tail`、`app.wasm-alias/sum-pair` 与 `app.wasm-alias/do` 的五项附带测试；`calcit.core/&fn:map`、`calcit.core/&fn:mappend`、`calcit.core/&fn:bind`、`calcit.core/&fn:apply` 与 `calcit.core/map-entries#survives-sort-and-indexing`（native、JS、WASM 与 Wasmtime WASI 0.3 command）；WASI capability / unknown-callee CLI 回归 |
| `struct-with` 的静态字段更新 | supported-with-type-evidence | 复用名义 Struct 的字段索引和布局校验；接收者、各更新值按源码顺序各求值一次，再进行不可变更新 | `test-types-inference.main/slice-with-contract` 的自定义实现测试，以及既有名义写入回归 |
| `&struct:matches?` 的具名定义与实例目标 | supported-with-type-evidence | 区分定义 ID 与实例指针；已知来源按模块内名义身份匹配，泛型定义匹配不证明具体 payload 类型。参数需证明为 Struct 或 StructDef；运行时创建局部定义仍不支持 | `test-traits.struct-left/Point` 的 `:struct-origin-portable` 五项共享测试，含别名、参数、不同来源和 `struct-match` |
| 作为参数传递的闭包 | supported-with-type-evidence | 调用点可静态特化，参数契约可调用（`E_WASM_CLOSURE_SPECIALIZATION`、`E_WASM_CALL_SPECIALIZATION`） | `calcit.core/&list:filter-pair#filters-key-value-pairs` 排除项 |
| 局部 `fn` 与立即调用的 `fn`的 `&` rest / `?` optional 参数，`&` 函数作为高阶回调 | supported | 按 native 绑定：缺省的 `?` 参数为 nil，`&` 参数收到剩余实参组成的 List；`foldl xs 0 +` 这类静态回调同样打包。函数体中的 `recur` 按同样规则重新绑定参数（`recur (&- n 1) n n` 把后两个实参打包进 `&` 参数） | `calcit.core/hint-fn#plain-call-into-rest-local`、`calcit.core/hint-fn#optional-then-rest-local`、`calcit.core/hint-fn#iife-with-rest-param`、`calcit.core/hint-fn#iife-rest-param-recur`、`calcit.core/hint-fn#rest-callee-in-hof` |
| 通过函数值（函数表）调用 `&` rest 函数 | supported | `calcit wasm`：`let ((f +)) (f 1 2)`、经定长 Fn 参数传入的 `defn first-of (a & _xs) a` 等调用按 native 打包剩余实参。实际经表调用时，N 个实参使用第 N 个函数表（N 不设上限，规范类型 `(f64 x N) -> f64` 按模块实际用到的最大 N 生成）；表中 `&` 函数的入口先打包 rest List，再调用原函数。`calcit wasi`：静态目标的具名别名可复用直接调用，未知的可达间接调用仍以 `E_WASI_COMMAND_INDIRECT` 拒绝 | `calcit.core/hint-fn#plain-call-rest-fn-value`、`calcit.core/hint-fn#plain-call-many-args-fn-value`（具名别名已回放 WASI，未知表调用仍受限） |
| 固定参数特化闭包与 `loop` 中的 `recur`，回边改变名义实现身份 | supported-with-type-evidence | 以调用实参证据开始特化循环；回边带来新的 exact 实现身份时，参数旁的 selector 按实际身份分派（每个参数至多 8 个），`if` 分支汇合同理（`E_WASM_CALL_SPECIALIZATION`） | `calcit.core/foldl-compare#typed-number-exits`、`test-traits.main/closure-recur-switch#two-iterations`、`test-traits.main/loop-switch#two-iterations`、`test-traits.main/branch-open-public#true-impl` |
| 嵌套 `fn` / `defn` 闭包值、带 `?` / `&` 参数的特化闭包中的 `recur` | unsupported | | `calcit.core/&list:apply#preserves-homogeneous-function-result-types` 排除项 |
| `try` | unsupported（WASI/WASM 退役） | `E_WASM_UNSUPPORTED_TRY`：可恢复错误使用 `Result :ok` / `Result :err` 与 `match`、`try-parse-*`、`try-decode-map-as` 等声明接口，不可恢复缺陷使用 `raise` / `unreachable!` / trap；WASI server 目标不编译业务 `try` | `tests/wasi_command_component_cli.rs` 与 `calcit.core/&number:rem#rejects-zero-and-non-safe-integers` 排除项 |
| 日志 Unit 返回与零/单参数 stdout | supported | core 宿主导入 `io.log_line(channel: f64, ptr: f64) -> ()`：channel `1/2` 分别表示 stdout/stderr，ptr 指向 tagged heap string；宿主每次写一行（含换行，空字符串也要输出空行），失败应抛错，不返回成功状态。WASI 使用标准通道 | `calcit.core/dbg#registered-stdout-returns-unit`、`registered-log-callback-returns-unit` |
| stderr 与多参数单行日志 | supported | core 以 `channel=1/2` 区分 stdout/stderr，每次表层调用只发出一次 host call；WASI 使用标准通道。全部参数先按从左到右顺序求值，再格式化输出 | `calcit.core/dbg#registered-stderr-returns-unit`、`registered-log-arguments-evaluate-once-in-order` |
| 局部 `ref`、`add-watch!` / `remove-watch!` | unsupported | 局部 Ref 与观察者；已有数值 `defref` 全局不在此排除范围 | `calcit.core/reset!#returns-assigned-scalar` 排除项 |
| 运行时 quote 值、`macroexpand`、`format-to-lisp` 非静态表达式 | unsupported | 宏在编译前展开，不受影响 | `calcit.core/deftrait#requires-head-accepts-bare-and-quoted`、`calcit.core/data-view#classifies-symbol-struct-ref` 排除项 |
| `turn-tag` / `to-tag` 运行时转换 | unsupported | 需要运行时 tag interning（`E_WASM_TAG_CONVERSION`） | `calcit.core/filter-map-kv#preserves-captured-generic-relations` 排除项 |
| `format-cirru-edn`、`try-parse-cirru-edn-as` | supported-with-type-evidence | 需要闭合静态类型与数据形状（`E_WASM_EDN_*`），见 [WASM 编译与验证](../../scripts/wasm-validation.md#cirru-edn-格式化边界) | `scripts/check-parse-boundary.mjs` |
| `decode-map-as`、`try-decode-map-as`、`encode-map-as`、`parse-cirru-edn-as` | supported-with-type-evidence | `decode-map-as` / `try-decode-map-as` / `encode-map-as` 复用闭合 `DataShapeGraph` 编解码受支持的标量种类（`Nil`、`Unit`、`Bool`、`Number` 与数值 refinement、`String`、`Tag`）、开放原样透传节点 `Dynamic`（含声明为 `Dynamic` 的 Struct 字段，编码时无需标量类型证据）、容器（`List`、`Tuple`、`Set`、`Map`）及名义 `Struct` / `Enum` / `Option` / `Result`；`parse-cirru-edn-as` 支持上述闭合标量、`List`、标量键 `Map` 及 `Struct` / `Enum`。编译期排除项与运行期失败严格区分：不支持的图节点（`Union`、`Ref`、`Symbol`、`Buffer`、`CirruQuote`，以及 `parse-cirru-edn-as` 中的 `Tuple` / `Set`）在 WASM 编译期以 `E_WASM_EDN_SHAPE` 拒绝模块，`decode-map-as` / `try-decode-map-as` 中缺少具体静态 `:tag` 证据的开放 `Tag` 节点，以及 `encode-map-as` 中缺少具体静态类型证据的开放 `Nil` / `Unit` / `Bool` / `Number` / `Numeric` / `Tag` 标量节点或开放 `Option` 节点在编译期以 `E_WASM_NIL_TYPE_EVIDENCE` 拒绝，共享节点展开超限在编译期以 `E_WASM_EDN_DEPTH` 拒绝；已通过编译的受支持形状在运行期遇到值或范围不匹配时，`try-decode-map-as` 返回 `Result :err`，`decode-map-as`、`encode-map-as` 与 `parse-cirru-edn-as` 在运行期 trap | `calcit.core/decode-map-as#decodes-struct-and-option`、`calcit.core/try-decode-map-as#result-contract`、`calcit.core/encode-map-as#round-trips-struct-and-option`、`calcit.core/encode-map-as#encodes-structs-inside-enum-payloads`、`calcit.core/try-parse-cirru-edn-as#parse-form-inside-typed-nth` |
| `json-parse`、`try-parse-json`（`String .parse-json`）、`json-stringify`、`json-pretty` | supported-with-type-evidence | `json-stringify` 与 `json-pretty` 支持闭合静态类型（`Nil`、`Bool`、`Number` 与数值 refinement、`String`、`Tag`、`List`、`Tuple`、`Set`、`:tag` 或 `:string` 键的 `Map`、名义 `Struct` / `Enum`，以及 `encode-map-as` 生成的结构化 `Map`），开放 `Dynamic` 输入在编译期报告 `E_WASM_EDN_TYPE`，非 `:tag` / `:string` 的 Map 键报告 `E_WASM_EDN_MAP_KEY`，非有限浮点数在运行期 trap；`json-parse` 与 `try-parse-json`（`.parse-json`）将 JSON 文本解析为 Calcit 值（JSON 对象键映射为编译期已登记的 `Tag`），可与 `decode-map-as` / `try-decode-map-as` 组合完成闭合类型边界解码 | `calcit.core/try-parse-json#result-method-contract`、`calcit.core/encode-map-as#round-trips-json-codec` |
| `&list:first`、`&list:last`、`&map:get`、`&str:first`、`&str:nth`、`&struct:definition`、`&get-env`、`&parse-float` | supported | 直接返回 `Option`：缺失为 `Option :none`；`&get-env` 在 core 目标需要宿主 `io.get_env`，在 WASI `--world http` 下以 `E_WASI_HTTP_CAPABILITY` 拒绝 | core `:tests`，`calcit.core/&get-env#missing-variable-is-none` 在 core 重放中按宿主排除 |
| `&map:destruct`、`&set:destruct` | supported | 直接返回 `MapDestruct<K,V>` / `SetDestruct<T>`：`:some` 的独立字段保留成员与剩余集合类型，空集合为对应的 `:none` | 两个原语与公开解构函数的 core `:tests` |
| `=` / `&=` / `not=`、Map 键操作与 Set 构造／成员／并差交运算 | supported-with-type-evidence | 顶层相等操作数、Map 键与 Set 元素类型（以及作为 Map 键或 Set 元素的列表、Tuple、Map 和 Set 的递归成员）必须能排除不同标量种类的数值表示冲突（如 `Number`、`Bool`、`Nil`、`Unit`、`Tag` 之间的擦除碰撞）；列表与 Tuple 的相等比较也会递归检查静态已知的元素类型（定长列表／Tuple 字面量对应位置为不同具体种类时直接收敛为不相等，已知具体元素标量种类可能发生擦除碰撞时报告 `E_WASM_NIL_TYPE_EVIDENCE`）；静态已知属于不同具体标量种类时直接收敛为不相等／不命中，至少一侧为已知非标量堆类型时按运行时堆头校验安全分区（其中 Set 与 Map 仍检查成员／键类型不碰撞）。缺少安全区分证据时，`=` / `&=` / `not=` 报告 `E_WASM_NIL_TYPE_EVIDENCE`（若涉及不安全 Set 成员或 Map 键类型则报告 `E_WASM_SET_ELEMENT_KIND`），Map 与 Set 操作报告 `E_WASM_SET_ELEMENT_KIND` | `calcit.core/=#distinguishes-disjoint-scalar-kinds`、`calcit.core/exclude#distinguishes-disjoint-scalar-kinds` 与 WASM / WASI CLI 拒绝用例 |
| 默认比较器的 `sort` | supported（静态证明的同种标量/字符串列表或 Tuple） | Tag 及开放/异构元素类型仍需显式两参数比较器 | `calcit.core/sort#orders-list-with-default-comparator`、`calcit.core/sort#sorts-nan-after-numbers` |
| `&read-dir` | unsupported | 旧目录原语仅用于 native 和配置了目录注入的 Node 宿主。在 WASI 使用 `FsPath .read-dir` 及其支持的文件系统边界 | `calcit.core/&read-dir#lists-sorted-paths-with-optional-recursion` 排除项 |
| 宿主能力（文件、时钟、随机数、进程参数、环境变量） | unsupported（core 目标 / WASI `--world http`） | core 目标报 `E_WASM_CAPABILITY`；`calcit wasi` 默认 `command` world 按所选边界支持，而 `--world http` 以 `E_WASI_HTTP_CAPABILITY` 拒绝这些仅限 `command` 的能力（仅保留 `println` / `eprintln` / `echo` 日志） | `calcit.core/wait-ms#positive-succeeds` 排除项 |
| JS FFI 定义、`unsafe-coerce` | unsupported | `E_WASM_UNSUPPORTED_JS_FFI` | `calcit.core/starts-with?#supports-tag-and-string-prefixes` 排除项 |
| `defwasm-import` / `defwasm-export` | supported-with-type-evidence | 显式 host ABI 边界（参数、rest 与返回值不得出现 `Dynamic`，否则在预处理期报告 `E_WASM_BOUNDARY_DYNAMIC`） | `scripts/wasm-validation.md`、`src/bin/cr_tests/type_fail.rs` |

完整的逐测试清单是 `scripts/core-tests-exclusions.cirru` 的 `:wasm` 与 `:wasi` 部分，原因以 `unsupported`、`parity`、`host`、`replay` 开头；`node scripts/run-core-tests.mjs --backend wasm --report-unexpected-pass` 列出已经可以移出清单的测试。

## 没有 WASM lowering 的 Proc

调用下列内建 Proc 时 WASM 报告 `unsupported proc in WASM: <name>`。`node scripts/run-core-tests.mjs` 校验排除清单中出现的每个此类 Proc 都列在这里。

```text wasm-unsupported-procs
&buffer
&cirru-nth
&cirru-quote:to-list
&cirru-type
&data-to-code
&display-stack
&extract-code-into-edn
&get-calcit-backend
&get-calcit-running-mode
&get-def-doc
&get-def-schema
&get-os
&inspect-methods
&methods-of
&number:format
&read-dir
&read-file
&reset-gensym-index!
&struct-def:new
&try-parse-cirru
&try-parse-cirru-edn
&try-parse-cirru-list
deftype-slot
format-cirru
format-cirru-one-liner
format-to-cirru
generate-id!
is-spreading-mark?
parse-cirru
parse-cirru-edn
parse-cirru-list
ref
to-lispy-string
turn-symbol
write-file
```

## 诊断

`E_WASM_*` 诊断以编号开头，随后写出涉及的定义或构造。新增编号时同步更新下表；`cargo test` 校验源码中的每个编号都列在这里。

| 编号 | 含义 | 处理方式 |
| --- | --- | --- |
| `E_WASM_TARGET` | 未知的 `--target`，只接受 `core` 或 `wasi` | 修正命令参数 |
| `E_WASM_BOUNDARY` | 未知的 `--boundary`，只接受 `native` 或 `component` | 修正命令参数 |
| `E_WASM_CAPABILITY` | 目标不提供所需宿主能力，或 `wasi` 目标出现未登记的自定义 import | 改用目标提供的能力，或换用 native / JS |
| `E_WASM_BOUNDARY_DYNAMIC` | `defwasm-export` 或 `defwasm-import` 在参数、rest 或返回类型中声明了 `Dynamic` | 改为具体的 WASM ABI 可表达类型（例如 `Number`、`String`、`Bool`、`Option`、`Result` 或名义 `Struct`/`Enum`） |
| `E_WASM_NIL_TYPE_EVIDENCE` | 开放类型或可空拼接上无法区分 nil、false、Unit、Tag 与 Number 的标量表示（含 `nil?`、`=` / `&=` / `not=` 及 `encode-map-as`）；数值排序操作数缺少 Number 证据 | 缺失值与判等边界使用具体闭合类型或 `Option`（可空返回值优先用 `nil->option`）；数值比较先解构／检查输入并证明为 Number，Option 本身不证明 Number |
| `E_WASM_TRAIT_TYPE_EVIDENCE` | trait 调用的接收者没有具体类型 | 给接收者写出具体类型 |
| `E_WASM_TRAIT_RESOLUTION` | 已知接收者类型下找不到 trait 实现 | 检查 `impl-traits` 是否覆盖该类型 |
| `E_WASM_TRAIT_CALL` | `&trait-call` 参数不是 trait、方法 tag 与接收者 | 修正调用形式 |
| `E_WASM_CALL_SPECIALIZATION` | 调用点特化需要固定、已知的 arity，或特化展开中再次调用自身（尾部 `recur` 不在此列，见上方的特化循环；但回边缺少类型证据、或一个循环参数带来超过 8 个实现身份时也报此错误）；也包括把运行时 List 以 `& xs` 展开传给 proc：仅当它是传给 `&list:concat` 的最后一个参数时支持，其他 proc 或非末尾位置都会报此错误（`([] ...)` 字面量会按普通参数展开） | 避免可变参数调用；非尾部的递归高阶调用改写为尾部 `recur`；逐个写出参数，或使用 `([] ...)` 字面量 |
| `E_WASM_CLOSURE_SPECIALIZATION` | 闭包参数的静态契约不可调用，或闭包逃逸出特化调用 | 直接传入具名函数或固定签名的 `fn` |
| `E_WASM_TAG_CONVERSION` | `turn-tag` / `to-tag` 需要运行时 tag interning | 使用字面量 tag |
| `E_WASM_SET_ELEMENT_KIND` | Set 元素或 Map 键操作缺少静态类型证据来排除不同标量种类的数值表示冲突 | 在构造、查询或集合运算前收窄元素与键类型，或使用 native / JS |
| `E_WASM_STRUCT_IDENTITY` | Struct 定义未在编译产物中登记，或来源判断缺少区分定义 ID 与实例指针的类型证据 | 通过具名 `defstruct` 构造；目标参数声明为 Struct 或 StructDef |
| `E_WASM_UNSUPPORTED_JS_FFI` | 定义只有 JavaScript 实现，或 `unsafe-coerce` 无法保持值身份 | 提供可 lowering 的实现 |
| `E_WASM_UNSUPPORTED_TRY` | 使用了 `try` 语法；WASM 与 WASI server 目标已退役 `try` 异常恢复流 | 可恢复错误改用 `Result<T, E>`（`Result :ok` / `Result :err` 与 `match`）；不可恢复不变式失败使用 `raise` trap，并将必须验证 trap 的旧测试留在 native / JS |
| `E_WASM_EDN_TYPE` | `format-cirru-edn`、`json-stringify` 或 `json-pretty` 的输入类型不闭合 | 写出闭合类型 |
| `E_WASM_EDN_SHAPE` | 解析缺少预处理生成的数据形状 | 使用 `try-parse-cirru-edn-as` 并给出目标类型 |
| `E_WASM_EDN_FORMAT_MODE` | 容器格式化只支持省略或字面量 `true` 的 inline 参数 | 去掉运行时布局参数 |
| `E_WASM_EDN_MAP_KEY` | Map key 类型无法保持 native 的排序，或 JSON 对象键不是 `:tag` / `:string` | 使用受支持的标量 key |
| `E_WASM_EDN_STRUCT_SHAPE` / `E_WASM_EDN_ENUM_SHAPE` | Struct / Enum 的类型参数个数不符 | 修正类型标注 |
| `E_WASM_EDN_DEPTH` | 数据形状嵌套过深 | 拆分数据结构 |
| `E_WASM_EDN_SYNTAX`、`E_WASM_EDN_TAG`、`E_WASM_EDN_ENUM`、`E_WASM_EDN_RANGE` | 运行时解析失败：语法错误、未知 tag、Enum 不匹配、数值越界；返回 `Result :err` | 按 Result 分支处理 |
| `E_WASM_EDN_INPUT_LIMIT`、`E_WASM_EDN_TOKEN_LIMIT`、`E_WASM_EDN_MAP_LIMIT` | 运行时输入超过 64 KiB、List 4096 项或 Map 2048 项；返回 `Result :err` | 拆分输入 |
| `E_WASI_WORLD` | 未知的 `--world`，或 `--world http` 与 `--boundary native` 组合 | `--world` 仅支持 `command` 或 `http`，且 `http` 必须使用 `--boundary component` |
| `E_WASI_HTTP_ENTRY` | HTTP 入口不是单参数函数，或缺少精确 `Fn HttpRequest (-> (Result HttpResponse String))` schema | 为 `:init-fn` 声明匹配的入参与返回值类型 |
| `E_WASI_HTTP_EXPORT` | 在 `--world http` 下额外声明通用 `defwasm-export` | HTTP proxy world 只导出 `wasi:http/incoming-handler@0.2.12`，移除多余 `defwasm-export` |
| `E_WASI_HTTP_CAPABILITY` | 在 `--world http` 可达闭包中调用仅限 `command` 的能力（如 CLI 参数、环境变量、filesystem、stdin、`quit!`、时钟、阻塞等待或安全随机数） | 保留在 `command` world 或改用纯 HTTP 请求/响应与 stdio 日志逻辑 |

## 维护

新增 WASM 能力的 PR 同步更新本页：表层构造表、`wasm-unsupported-procs` 列表和诊断表，并移除 `scripts/core-tests-exclusions.cirru` 中对应的排除项。

诊断覆盖检查只读取“诊断”章节表格的“编号”列；正文、其他表格或说明列中引用的编号不算已登记。同一格可以列出多个分别用反引号包裹的编号。

## 源码布局

WASM 后端与 FFI ABI 的实现集中在 `src/wasm/`，对外公开路径保持不变：

| 文件 | 公开路径 | 需要 `wasm` feature |
| --- | --- | --- |
| `src/wasm/emit_wasm.rs` 及 `emit_wasm/` | `calcit::codegen::emit_wasm` | 是 |
| `src/wasm/wasm_cli.rs` | `calcit::wasm_cli` | 是 |
| `src/wasm/ffi_abi.rs`、`ffi_async.rs` | `calcit::ffi_abi`、`calcit::ffi_async` | 否（仅非 wasm32 目标才编译；不需要 wasm feature） |
| `src/wasm/ffi_interface_ir.rs` | `calcit::ffi_interface_ir` | 否 |

`src/wasm/mod.rs` 是统一入口 `calcit::wasm`：按上表的 feature 条件重新导出 `WasmTarget`、`WasmBoundary`、`emit_wasm`、`validate_wasm_target`、`validate_wasm_boundary`、`check_wasm_command_component`、`WasmCliOptions`、`run`，以及三个 FFI 模块。这些模块仍通过 `src/lib.rs` 与 `src/codegen.rs` 里的 `#[path]` 声明挂载，所以 FFI 模块不会被 `wasm` feature 门控。

本仓库内 `src/wasm/` 之外的调用方（`src/bin/`、`tests/` 等）统一通过 `calcit::wasm` 使用这些条目，例如 `calcit::wasm::WasmCliOptions`、`calcit::wasm::run`、`calcit::wasm::ffi_abi::…`。上表中的旧公开路径继续保留为兼容别名，供外部使用者过渡；是否移除属于后续非 patch 版本的决定，patch 版本不删除。

`cargo test boundary_tests` 会扫描 `src/`、`tests/` 与 `benches/`，在以下情况失败：直接引用 `src/wasm/` 下的实现文件，或把 `emit_wasm`、`wasm_cli`、`ffi_abi`、`ffi_async`、`ffi_interface_ir` 作为路径段使用而前面不是 `wasm::`。`src/lib.rs` 与 `src/codegen.rs` 的 `#[path]` 挂载是唯一的允许项。新增对外条目时同时更新 `src/wasm/mod.rs` 的重新导出。

### Host facade

`src/wasm/host.rs` 是该目录依赖主 crate 内部的唯一入口：`calcit`、`data`、`snapshot`、`builtins`、`runner`、`program`、`util`、`call_stack`、`codegen`、`js_ffi_source`，以及 CLI 驱动用到的 `ProgramEntries`、`load_core_snapshot` 等根级条目，都在这里用 `pub(crate) use` 重新导出。`src/wasm/` 下的其他文件只写 `crate::wasm::host::…`，不直接写 `crate::<主 crate 内部模块>`，例如 `use crate::wasm::host::calcit::Calcit;`、`crate::wasm::host::runner::preprocess::infer_static_type_from_expr`。没有 `wasm` feature 时只导出 FFI interface IR 需要的 `calcit`、`data`、`snapshot`，其余条目按 feature 门控。

`wasm::boundary_tests` 中的 `wasm_dir_reaches_main_crate_only_through_host` 会扫描 `src/wasm/` 下所有 `.rs` 文件（忽略注释与字符串，展开多行 `use crate::{…}`），除 `host.rs` 外（`mod.rs` 也不例外），出现 `crate::` 后接 `wasm`、`wasm_cli`、`ffi_abi`、`ffi_async`、`ffi_interface_ir`、`codegen::emit_wasm` 之外的名字就会失败；裸 `crate::codegen`（含 `use crate::codegen;`、分组与 `as`）同样会失败。需要新的主 crate 条目时，先在 `host.rs` 增加重新导出，再在调用处经由 `crate::wasm::host` 使用。

## 内存模型与 Arena 回收

WASM/WASI 运行时采用分层线性内存布局，将长期状态与单次请求临时分配隔离回收：

- **三段式线性内存区域**：静态常量池位于 `[HEAP_BASE, static_end)`；若程序包含 `defref`，其后预留两个各 1 MiB 的持久区半空间（Semi-Space A/B），否则直接衔接请求区；其后的 `[arena_base, ...)` 为请求级 Scratch Arena，由 `__heap_ptr` 线性 bump 分配，并通过只读 global `__arena_base` 与（含 `defref` 时导出的）`__persistent_ptr` 向宿主暴露水位线状态。
- **Copy-on-Commit 与半空间压缩**：
  - `defref` 的静态初始值（`String`、`Tag`、`Number`、`Bool`、`Nil` 与空容器 `[]`/`{}`/`#{}`）留在静态池；编译期已知为 `Ref <Scalar>` 或写入值为静态标量时，`reset!` / `swap!` 绕过 `__rt_commit_defref`，半空间压缩也会跳过静态标量 `Ref`。
  - 非标量写入进入 `__rt_commit_defref`，在 Scratch Arena 之上的临时开放寻址 forwarding table 对共享子图做 DAG 去重晋升而不修改 Scratch 对象的 `HEAP_MAGIC` 头，深拷贝晋升可达子图（`String`、`Buffer`、`List`、`Map`、`Set`、`Struct`、`Enum` 及 HAMT 节点）并复用已在目标半空间的节点。
  - 当本次 commit 确实增加了活跃半空间分配（`pers.ptr > ptr_before`）且超出相对上次压缩剩余半空间的一半（`next_compact_ptr = new_ptr + ((new_end - new_ptr) >> 1)`）或发生 turn 内临时溢出时标记压缩；压缩通过 `stage_ptr`/`stage_limit` 预演、原子提交 `atom` 全局量与元数据、并通过两遍遍历重定位仍保活的 Scratch 返回值链（`scratch_roots_head`）与当前返回值可达的旧半空间子指针，确保之前导出尚未被 `__arena_reset()` 释放的 Scratch 返回图（及其引用的旧半空间子对象）在多次跨 turn 压缩后依然有效；压缩途中若因容量溢出 trap，旧半空间根保持完整可读，且仅当允许安全回退（`can_rewind_stale`）且 `pers.scratch_end` 之上的临时 forwarding table 仍位于 Scratch 顶部（无宿主通过 `__str_new` 追加新分配）时才回收该失败工作区，否则仅清零 forwarding 状态并继续保留宿主新写入的参数或已发布根仍引用的 Scratch 对象，重试时仍会正确报告或完成压缩。
- **O(1) Turn 级水印复位、堆返回值保活与异常恢复**：
  - Core `defwasm-export` 在退出时：若返回值为静态标量（或位于持久区/静态池/进入前 Scratch 的既有值），立即将 `__heap_ptr` 复位为进入时的水印并弹出该水印之上的保留根链表节点；若返回值是本 turn 在 Scratch Arena 新分配的堆对象，则在持久堆模式下压入 `scratch_roots_head` 保持活跃直至宿主显式调用 `__arena_reset()`（直接返回 `defref` 中的裸持久指针应先读取或随 `__arena_reset()` 生命周期管理，跨压缩继续读取则请通过新分配的 Scratch 容器返回或重新读取 `@*ref`）。`__arena_reset()` 会清零 `active_turns` / `host_call_depth` / `scratch_roots_head`、回收遗留转发工作区（同样仅在其位于栈顶时）、执行待定半空间压缩并将 `__heap_ptr` 重置回 `__arena_base`。
  - Core trap 恢复遵循明确的宿主契约：由于 WebAssembly trap 会直接展开回宿主而不执行 WASM epilogue，仅纯 WASM 与内置 `io::*` 叶子导入（不包裹 `host_call_depth`）在捕获 trap 后无需 `__arena_reset()` 即可由下一次顶层导出通过 `__rt_turn_enter`（`host_call_depth == 0`）全量恢复；对可重入的 `defwasm-import` 仍保留 `host_call_depth` 以支持回调嵌套，因此**宿主一旦捕获来自 `defwasm-import` 的 trap，必须在继续复用实例时显式调用 `__arena_reset()`** 以恢复最外层 turn 记账并执行待定压缩（包括半空间溢出 `needs_compact == 2` 时暂停单调用 Scratch 回退以避免覆写溢出到 Scratch 的 `*ref` 目标，直到 `__arena_reset()` 将其压缩迁出并复位）。
  - Component 模型根据 `turn_mark`、`live_above_mark` 与 `total_live` 进行精确回收：无 `post-return` 的同步导出、stackless/stream 异步回调在进入 `WAIT` 状态前（此时调用 turn-exit 释放 `active_turns` 并在 `callback` 重新进入）与完成态以及 `cabi_post_*` 在最外层退出（`active_turns == 0` 且执行完待定压缩）后按如下条件判定：
    - 若 `total_live == 0`，清空空闲链表（`free_head = 0`）并将 `__heap_ptr` 重置回 `__arena_base`；
    - 否则若存在 `turn_mark` 且 `live_above_mark == 0`，摘除所有起始地址 `>= turn_mark` 的空闲块并将 `__heap_ptr` 回退到 `turn_mark`；
    - 否则（`live_above_mark > 0`），两种回收都延迟执行，直到后续 turn 退出、`post-return` 或资源释放使上述条件满足。
  - 此外，当外层 Component 导出处于活跃期（`active_turns > 0`）且未挂起新的 `live_above_mark` 缓冲时，同步无 `post-return` 导出会在返回时将 `__heap_ptr` 与空闲链表回退至本调用的 `call_mark`；stackless 宽参数异步导出使用 `cabi_realloc` 分配间接参数记录并在子任务进入 `STARTED`、立即返回或终态时以 `cabi_free` 释放；被取消的 stackless async 任务在 `RETURNED` 状态会先执行结果析构（`drop`）再回收缓冲，仅取消未返回时只释放结果区。
- **已证明封闭标量与紧凑名义布局**：
  - 对静态类型已证明为封闭的 `Bool`、`Tag`、`Unit`、整型细化类型（`Int8`..`Int32`、`UInt8`..`UInt32` 用 `i32`；`Int64`、`UInt64` 用 `i64`）以及已知非空堆指针，WASM 代码生成器在内部局部槽位与直调函数签名中采用原生 `i32`/`i64` 表示，并在 `if` / `not` / `=` / `identical?` / `let` / 已知容器方法调用与 `to-string` 等路径上省去运行时 `HEAP_MAGIC` 探测；当带原生签名的函数经由 `funcref` 表 `call_indirect` 调用时，编译器自动插入 `(f64 x N) -> f64` 适配桩完成封箱与拆箱转换（`defwasm-export` 与 `type-of` 对堆值的活跃区间与 `HEAP_MAGIC` 校验保持不变，含 `try-parse-json` 时 JSON 堆封箱标量同样兼容）。
  - 名义 `Struct` 采用 8 字节紧凑头 `[count:u32 @ 0][layout_id:u32 @ 4]`，各字段按自然对齐降序排列并对齐填充至 8 字节倍数（`Unit` 占 0 字节，`Bool`/`Int8`/`UInt8` 占 1 字节，`Int16`/`UInt16` 占 2 字节，`Tag`/`Int32`/`UInt32` 与已证明堆指针字段占 4 字节，一般 `Number`/开放字段与 `Int64`/`UInt64` 占 8 字节；空结构体最小负载为 8 字节，整体仍维持 `[-8, 0)` 分配头与 8 字节对齐）。只读静态描述符表记录每个 `layout_id` 的负载字节数、字段数与 `[offset:u16, kind:u8]` 元数据，供运行时晋升、重定位、哈希与结构相等精确读写并只追踪 `BoxedF64` 与非零 `HeapPtrI32` 字段。
  - `Enum`（含 `Option`、`Result` 与匿名 `::`）同样采用 `[count:u32 @ 0][tag_id:u32 @ 4]` 紧凑头，payload 从 `+8` 偏移连续存放，`0` 载荷变体负载为 8 字节。

## 限制

- host facade 检查不覆盖 `super::super::`、`use crate as c;` 与 `::crate::` 形式的别名路径。
- 边界检查忽略注释（含块注释）与字符串、字符字面量，并展开多行与嵌套的 `use` 分组，要求这五个模块名前面是 `calcit::wasm::`、`crate::wasm::`，或已用 `use calcit::wasm;` / `use calcit::wasm::{self, …}` 引入的 `wasm::`；它不解析 `as` 别名、`super::` 和宏生成的路径。
- 不带编号的 `unsupported ... in WASM` 诊断尚未统一格式，不一定包含源位置与缺失的证据种类。
- `wasm-unsupported-procs` 列表只校验排除清单出现过的 Proc，没有 core 测试覆盖的 Proc 靠人工维护。
- `update` 的接收者为 Struct 时，仅支持静态证明的具名 Struct 与字面量 tag key；开放类型或动态 key 的 Struct 接收者在 WASM 上仍会 trap，native 会改写对应字段。
- 通过函数值调用时，实参个数少于 `&` 之前的参数个数（含 `?` 参数），或与定长函数的参数个数不同，都会 trap；缺省的 `?` 参数不会绑定为 nil。
- `&compare` 在 WASM 上对两个字符串按字典序比较，字符串与非字符串比较时字符串更大；tag、list 等其他堆类型之间仍按数值比较，与 native 的跨类型全序不同。
- `=`、`&compare` 和缺少具体标量证据的 `type-of` 仅把落入活跃区间（静态池、当前活跃持久区半空间与 `[arena_base + 8, __heap_ptr)`）且 `ptr-8` 处为 `HEAP_MAGIC` 的整数视作堆值，开放或可空边界下同值标量与指针仍可能混淆，具名标量的静态证据不受影响。
- 单个持久区半空间容量上限为 1 MiB，全部活跃 `defref` 可达对象图超出该上限时会触发 WASM trap。
- 已证明为整型细化类型（`Int8`..`UInt64`）的 WASM 内部槽位与紧凑 `Struct` 字段使用原生整型表示，会将 `-0` 规范化为 `0`（数值相等与比较语义不变，需要保留 IEEE 754 负零符号时请使用通用 `Number` 或 `Float64`）。
- `defref` 晋升与半空间压缩的图遍历递归深度上限为 256 层，超过该深度时会触发 WASM trap。
