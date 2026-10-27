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

本页说明一段 Calcit 代码能否由 `calcit wasm`（browser/embedded core module）与 `calcit wasi`（WASI command）编译。两个命令共享加载、预处理、target 校验与 codegen，下面的状态对两者相同，个别行注明了两者的差异；WASI 宿主能力另见 [宿主能力边界](host-capability-boundary.md)，Component 类型见 [WASM Component 边界](wasm-component-boundary.md)。

## 状态

- **supported**：直接编译，结果与 native 一致，由 core `:tests` 在 WASM 上回放验证。
- **supported-with-type-evidence**：只有预处理已经证明了闭合的具体类型时才能编译，缺少证据时报告对应的 `E_WASM_*` 诊断。补充证据的方式是给参数、返回值或局部绑定写出具体类型，或改用已证明的 nominal 构造，而不是把类型放宽为 Dynamic。
- **unsupported**：当前 WASM 后端明确拒绝，诊断为 `unsupported ... in WASM` 或 `E_WASM_CAPABILITY` 等；这是目标后端的支持边界，不改变该构造在 native 与 JS 上的语义。

普通依赖里不受支持的定义会编译为 trap 桩，只有调用到时才 trap；显式导出与 command 入口在编译期报告。生成了产物不代表可用，应实际调用入口验证。

## 表层构造

| 构造 | 状态 | 证据或说明 | 验证 |
| --- | --- | --- | --- |
| `defn`、固定参数、`let`、`if`、`recur` 尾递归 | supported | 参数与返回值为 f64 值 ABI | core `:tests`，如 `calcit.core/recur` |
| Number、Bool、nil、String、tag 字面量 | supported | nil、false 与 0 共用 f64 表示，见下文类型判断的说明 | core `:tests` |
| 具体标量输入的 `type-of`、`number?`、`bool?` | supported | 已证明的 Number（含数值 refinement）、Bool、Nil、Unit 优先使用真实类型证据，实参仍求值一次；不会将 false／nil／Unit 的零值重新解释为 Number，合法 Number 也不会被同值堆地址覆盖 | `calcit.core/number?` 的 `:scalar-kind-evidence` 测试与 WASM 参数失败路径回归 |
| `nil?`、Option 在开放类型上的判断 | supported-with-type-evidence | 需要具体类型区分 nil、false 与 0（`E_WASM_NIL_TYPE_EVIDENCE`） | `calcit.core/update-in#updates-collection-and-missing-paths` 等开放遍历排除项；静态 Map 与字面量空路径用例已通过 |
| 数值排序比较 `&<`、`&>` 及其表层调用 | supported-with-type-evidence | 操作数需要 Number 证据；已知非 Number 保留实参求值后 trap，开放类型在 scalar 擦除前报告 `E_WASM_NIL_TYPE_EVIDENCE`。具体 Number 的泛型调用复用调用点特化；`identical?` 不受数值限制 | `calcit.core/contains?` 的 `:enum-numeric-evidence` 测试与 WASM 宿主失败路径回归 |
| List / Map / Set / Struct / Enum 值与 core 方法 | supported | 线性内存 + bump allocator；`#{}` 字面量不去重 | core `:tests` |
| `update` 作用于 List / Map / Enum | supported | 按运行时类型分派：List 下标越界或 Map key 不存在时原样返回且不调用回调；Enum 下标越界时 trap，对应 native 的 range error | `calcit.core/update#updates-list-by-index-and-skips-missing-slots` |
| 已证明的普通方法调用、trait 方法 | supported-with-type-evidence | 普通方法复用接收者的实际实现：List、Map、Set 等内建类型使用固定方法表，成员类型仍可为泛型；名义值还需证明实际附带的实现身份。显式 trait 调用仍需具体类型（`E_WASM_TRAIT_TYPE_EVIDENCE`、`E_WASM_TRAIT_RESOLUTION`） | `test-types-inference.main/find-last-positive-index#typed-number-receiver`、`find-last-matching-index#keeps-number-and-string-element-types`；名义实现由 `:impl-origin-portable` 测试覆盖 |
| `slice`、`.slice` 与自定义 `Sliceable` | supported-with-type-evidence | 终点为 `Option<Number>`：`Option :some end` 指定终点，省略或 `Option :none` 取到末尾；接收者和实现需要可静态解析，保留 String 的 Unicode 标量单位与 List 成员类型 | `calcit.core/slice#option-endpoints-through-function-method-and-trait`、`test-types-inference.main/slice-with-contract` 的四项共享测试 |
| 固定参数函数、普通内联闭包与立即调用的尾部 Option 缺省 | supported-with-type-evidence | 省略实参补入真实名义 `Option :none`，不使用 nil；已知不可变泛型函数的局部别名在调用点取得参数证据后特化 | `calcit.core/slice#inline-option-omission-keeps-nominal-none`、`test-types-inference.main/slice-with-contract#custom-function-reference`、`test-types-inference.main/option-recur-sum#immediate-option-completion` |
| 已知目标的函数值与局部别名 | supported-with-type-evidence | 目标明确时复用具名调用的 lowering，支持别名链、词法遮蔽、闭包捕获、`map/foldl`、具名 rest / 字面量 spread；内联调用的回调实参若为明确的具名定义，也保留目标。函数值可保存并保持身份，实参按原顺序各求值一次；WASI 的未知间接目标与不支持的可达宿主能力仍拒绝 | `app.wasm-alias/bump`、`app.wasm-alias/sum-tail`、`app.wasm-alias/sum-pair` 与 `app.wasm-alias/do` 的五项附带测试；`calcit.core/hint-fn#local-hint-keeps-proven-function-alias`；WASI capability / unknown-callee CLI 回归 |
| `struct-with` 的静态字段更新 | supported-with-type-evidence | 复用名义 Struct 的字段索引和布局校验；接收者、各更新值按源码顺序各求值一次，再进行不可变更新 | `test-types-inference.main/slice-with-contract` 的自定义实现测试，以及既有名义写入回归 |
| `&struct:matches?` 的具名定义与实例目标 | supported-with-type-evidence | 区分定义 ID 与实例指针；已知来源按模块内名义身份匹配，泛型定义匹配不证明具体 payload 类型。参数需证明为 Struct 或 StructDef；运行时创建局部定义仍不支持 | `test-traits.struct-left/Point` 的 `:struct-origin-portable` 五项共享测试，含别名、参数、不同来源和 `struct-match` |
| 作为参数传递的闭包 | supported-with-type-evidence | 调用点可静态特化，参数契约可调用（`E_WASM_CLOSURE_SPECIALIZATION`、`E_WASM_CALL_SPECIALIZATION`） | `calcit.core/&list:filter-pair#filters-key-value-pairs` 排除项 |
| 局部 `fn` 与立即调用的 `fn`的 `&` rest / `?` optional 参数，`&` 函数作为高阶回调 | supported | 按 native 绑定：缺省的 `?` 参数为 nil，`&` 参数收到剩余实参组成的 List；`foldl xs 0 +` 这类静态回调同样打包。函数体中的 `recur` 按同样规则重新绑定参数（`recur (&- n 1) n n` 把后两个实参打包进 `&` 参数） | `calcit.core/hint-fn#plain-call-into-rest-local`、`calcit.core/hint-fn#optional-then-rest-local`、`calcit.core/hint-fn#iife-with-rest-param`、`calcit.core/hint-fn#iife-rest-param-recur`、`calcit.core/hint-fn#rest-callee-in-hof` |
| 通过函数值（函数表）调用 `&` rest 函数 | supported | `calcit wasm`：`let ((f +)) (f 1 2)`、经定长 Fn 参数传入的 `defn first-of (a & _xs) a` 等调用按 native 打包剩余实参。实际经表调用时，N 个实参使用第 N 个函数表（N 不设上限，规范类型 `(f64 x N) -> f64` 按模块实际用到的最大 N 生成）；表中 `&` 函数的入口先打包 rest List，再调用原函数。`calcit wasi`：静态目标的具名别名可复用直接调用，未知的可达间接调用仍以 `E_WASI_COMMAND_INDIRECT` 拒绝 | `calcit.core/hint-fn#plain-call-rest-fn-value`、`calcit.core/hint-fn#plain-call-many-args-fn-value`（具名别名已回放 WASI，未知表调用仍受限） |
| 固定参数特化闭包与 `loop` 中的 `recur`，回边改变名义实现身份 | supported-with-type-evidence | 以调用实参证据开始特化循环；回边带来新的 exact 实现身份时，参数旁的 selector 按实际身份分派（每个参数至多 8 个），`if` 分支汇合同理（`E_WASM_CALL_SPECIALIZATION`） | `calcit.core/foldl-compare#typed-number-exits`、`test-traits.main/closure-recur-switch#two-iterations`、`test-traits.main/loop-switch#two-iterations`、`test-traits.main/branch-open-public#true-impl` |
| 嵌套 `fn` / `defn` 闭包值、带 `?` / `&` 参数的特化闭包中的 `recur` | unsupported | | `calcit.core/&list:apply#preserves-homogeneous-function-result-types` 排除项 |
| `try` | unsupported | 失败只能 trap | `calcit.core/&number:rem#rejects-zero-and-non-safe-integers` 排除项 |
| 日志 Unit 返回与零/单参数 stdout | supported | core 宿主导入 `io.log_line(channel: f64, ptr: f64) -> ()`：channel `1/2` 分别表示 stdout/stderr，ptr 指向 tagged heap string；宿主每次写一行（含换行，空字符串也要输出空行），失败应抛错，不返回成功状态。WASI 使用标准通道 | `calcit.core/dbg#registered-stdout-returns-unit`、`registered-log-callback-returns-unit` |
| stderr 与多参数单行日志 | supported | core 以 `channel=1/2` 区分 stdout/stderr，每次表层调用只发出一次 host call；WASI 使用标准通道。全部参数先按从左到右顺序求值，再格式化输出 | `calcit.core/dbg#registered-stderr-returns-unit`、`registered-log-arguments-evaluate-once-in-order` |
| 局部 `ref`、`add-watch!` / `remove-watch!` | unsupported | 局部 Ref 与观察者；已有数值 `defref` 全局不在此排除范围 | `calcit.core/reset!#returns-assigned-scalar` 排除项 |
| 运行时 quote 值、`macroexpand`、`format-to-lisp` 非静态表达式 | unsupported | 宏在编译前展开，不受影响 | `calcit.core/deftrait#requires-head-accepts-bare-and-quoted`、`calcit.core/data-view#classifies-symbol-struct-ref` 排除项 |
| `turn-tag` / `to-tag` 运行时转换 | unsupported | 需要运行时 tag interning（`E_WASM_TAG_CONVERSION`） | `calcit.core/filter-map-kv#preserves-captured-generic-relations` 排除项 |
| `format-cirru-edn`、`try-parse-cirru-edn-as` | supported-with-type-evidence | 需要闭合静态类型与数据形状（`E_WASM_EDN_*`），见 [WASM 编译与验证](../../scripts/wasm-validation.md#cirru-edn-格式化边界) | `scripts/check-parse-boundary.mjs` |
| `decode-map-as`、`parse-cirru-edn-as` | unsupported | | `calcit.core/decode-map-as#decodes-struct-and-option` 排除项 |
| `&list:first`、`&list:last`、`&map:get`、`&str:first`、`&str:nth`、`&struct:definition`、`&get-env`、`&parse-float` | supported | 直接返回 `Option`：缺失为 `Option :none`；`&get-env` 在 core 目标需要宿主 `io.get_env` | core `:tests`，`calcit.core/&get-env#missing-variable-is-none` 在 core 重放中按宿主排除 |
| `&map:destruct`、`&set:destruct` | supported | 直接返回 `MapDestruct<K,V>` / `SetDestruct<T>`：`:some` 的独立字段保留成员与剩余集合类型，空集合为对应的 `:none` | 两个原语与公开解构函数的 core `:tests` |
| 默认比较器的 `sort` | unsupported | 需要显式两参数比较器 | `calcit.core/sort#orders-list-with-default-comparator` 排除项 |
| 宿主能力（文件、时钟、随机数、进程参数、环境变量） | unsupported（core 目标） | `E_WASM_CAPABILITY`；WASI 按所选边界单独支持 | `calcit.core/wait-ms#positive-succeeds` 排除项 |
| JS FFI 定义、`unsafe-coerce` | unsupported | `E_WASM_UNSUPPORTED_JS_FFI` | `calcit.core/starts-with?#supports-tag-and-string-prefixes` 排除项 |
| `defwasm-import` / `defwasm-export` | supported | 显式 host ABI，Number / String | `scripts/wasm-validation.md` |

完整的逐测试清单是 `scripts/core-tests-exclusions.cirru` 的 `:wasm` 与 `:wasi` 部分，原因以 `unsupported`、`parity`、`host`、`replay` 开头；`node scripts/run-core-tests.mjs --backend wasm --report-unexpected-pass` 列出已经可以移出清单的测试。

## 没有 WASM lowering 的 Proc

调用下列内建 Proc 时 WASM 报告 `unsupported proc in WASM: <name>`。`node scripts/run-core-tests.mjs` 校验排除清单中出现的每个此类 Proc 都列在这里。

```text wasm-unsupported-procs
&cirru-nth
&cirru-quote:to-list
&cirru-type
&data-to-code
&display-stack
&extract-code-into-edn
&format-ternary-tree
&get-calcit-backend
&get-calcit-running-mode
&get-def-doc
&get-def-schema
&get-os
&inspect-methods
&methods-of
&number:format
&read-file
&reset-gensym-index!
&struct-def:new
deftype-slot
format-cirru
format-cirru-one-liner
format-to-cirru
generate-id!
is-spreading-mark?
json-parse
json-pretty
json-stringify
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
| `E_WASM_NIL_TYPE_EVIDENCE` | 开放类型上无法区分 nil、false 与 0；数值排序操作数缺少 Number 证据 | 缺失值边界使用具体类型或 Option；数值比较先解构／检查输入并证明为 Number，Option 本身不证明 Number |
| `E_WASM_TRAIT_TYPE_EVIDENCE` | trait 调用的接收者没有具体类型 | 给接收者写出具体类型 |
| `E_WASM_TRAIT_RESOLUTION` | 已知接收者类型下找不到 trait 实现 | 检查 `impl-traits` 是否覆盖该类型 |
| `E_WASM_TRAIT_CALL` | `&trait-call` 参数不是 trait、方法 tag 与接收者 | 修正调用形式 |
| `E_WASM_CALL_SPECIALIZATION` | 调用点特化需要固定、已知的 arity，或特化展开中再次调用自身（尾部 `recur` 不在此列，见上方的特化循环；但回边缺少类型证据、或一个循环参数带来超过 8 个实现身份时也报此错误）；也包括把运行时 List 以 `& xs` 展开传给 proc：仅当它是传给 `&list:concat` 的最后一个参数时支持，其他 proc 或非末尾位置都会报此错误（`([] ...)` 字面量会按普通参数展开） | 避免可变参数调用；非尾部的递归高阶调用改写为尾部 `recur`；逐个写出参数，或使用 `([] ...)` 字面量 |
| `E_WASM_CLOSURE_SPECIALIZATION` | 闭包参数的静态契约不可调用，或闭包逃逸出特化调用 | 直接传入具名函数或固定签名的 `fn` |
| `E_WASM_TAG_CONVERSION` | `turn-tag` / `to-tag` 需要运行时 tag interning | 使用字面量 tag |
| `E_WASM_STRUCT_IDENTITY` | Struct 定义未在编译产物中登记，或来源判断缺少区分定义 ID 与实例指针的类型证据 | 通过具名 `defstruct` 构造；目标参数声明为 Struct 或 StructDef |
| `E_WASM_UNSUPPORTED_JS_FFI` | 定义只有 JavaScript 实现，或 `unsafe-coerce` 无法保持值身份 | 提供可 lowering 的实现 |
| `E_WASM_EDN_TYPE` | `format-cirru-edn` 的输入类型不闭合 | 写出闭合类型 |
| `E_WASM_EDN_SHAPE` | 解析缺少预处理生成的数据形状 | 使用 `try-parse-cirru-edn-as` 并给出目标类型 |
| `E_WASM_EDN_FORMAT_MODE` | 容器格式化只支持省略或字面量 `true` 的 inline 参数 | 去掉运行时布局参数 |
| `E_WASM_EDN_MAP_KEY` | Map key 类型无法保持 native 的排序 | 使用标量 key |
| `E_WASM_EDN_STRUCT_SHAPE` / `E_WASM_EDN_ENUM_SHAPE` | Struct / Enum 的类型参数个数不符 | 修正类型标注 |
| `E_WASM_EDN_DEPTH` | 数据形状嵌套过深 | 拆分数据结构 |
| `E_WASM_EDN_SYNTAX`、`E_WASM_EDN_TAG`、`E_WASM_EDN_ENUM`、`E_WASM_EDN_RANGE` | 运行时解析失败：语法错误、未知 tag、Enum 不匹配、数值越界；返回 `Result :err` | 按 Result 分支处理 |
| `E_WASM_EDN_INPUT_LIMIT`、`E_WASM_EDN_TOKEN_LIMIT`、`E_WASM_EDN_MAP_LIMIT` | 运行时输入超过 64 KiB、List 4096 项或 Map 2048 项；返回 `Result :err` | 拆分输入 |

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

## 限制

- host facade 检查不覆盖 `super::super::`、`use crate as c;` 与 `::crate::` 形式的别名路径。
- 边界检查忽略注释（含块注释）与字符串、字符字面量，并展开多行与嵌套的 `use` 分组，要求这五个模块名前面是 `calcit::wasm::`、`crate::wasm::`，或已用 `use calcit::wasm;` / `use calcit::wasm::{self, …}` 引入的 `wasm::`；它不解析 `as` 别名、`super::` 和宏生成的路径。
- 不带编号的 `unsupported ... in WASM` 诊断尚未统一格式，不一定包含源位置与缺失的证据种类。
- `wasm-unsupported-procs` 列表只校验排除清单出现过的 Proc，没有 core 测试覆盖的 Proc 靠人工维护。
- `update` 的接收者为 Struct 时在 WASM 上 trap，native 会改写对应字段。
- 通过函数值调用时，实参个数少于 `&` 之前的参数个数（含 `?` 参数），或与定长函数的参数个数不同，都会 trap；缺省的 `?` 参数不会绑定为 nil。
- `&compare` 在 WASM 上对两个字符串按字典序比较，字符串与非字符串比较时字符串更大；tag、list 等其他堆类型之间仍按数值比较，与 native 的跨类型全序不同。
- `=`、`&compare` 和缺少具体标量证据的 `type-of` 仍使用堆值判定（整数、位于已分配堆内、ptr-8 处为 `HEAP_MAGIC`）；没有 NaN-boxing，开放／可空边界仍可能无法区分同值标量与指针。具体 Number、Bool、Nil、Unit 的静态证据不受该猜测覆盖。
