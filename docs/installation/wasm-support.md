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

本页说明一段 Calcit 代码能否由 `calcit wasm`（browser/embedded core module）与 `calcit wasi`（WASI command）编译。两个命令共享加载、预处理、target 校验与 codegen，下面的状态对两者相同；WASI 宿主能力另见 [宿主能力边界](host-capability-boundary.md)，Component 类型见 [WASM Component 边界](wasm-component-boundary.md)。

## 状态

- **supported**：直接编译，结果与 native 一致，由 core `:tests` 在 WASM 上回放验证。
- **supported-with-type-evidence**：只有预处理已经证明了闭合的具体类型时才能编译，缺少证据时报告对应的 `E_WASM_*` 诊断。补充证据的方式是给参数、返回值或局部绑定写出具体类型，或改用已证明的 nominal 构造，而不是把类型放宽为 Dynamic。
- **unsupported**：当前 WASM 后端明确拒绝，诊断为 `unsupported ... in WASM` 或 `E_WASM_CAPABILITY` 等；这是目标后端的支持边界，不改变该构造在 native 与 JS 上的语义。

普通依赖里不受支持的定义会编译为 trap 桩，只有调用到时才 trap；显式导出与 command 入口在编译期报告。生成了产物不代表可用，应实际调用入口验证。

## 表层构造

| 构造 | 状态 | 证据或说明 | 验证 |
| --- | --- | --- | --- |
| `defn`、固定参数、`let`、`if`、`recur` 尾递归 | supported | 参数与返回值为 f64 值 ABI | core `:tests`，如 `calcit.core/recur` |
| Number、Bool、nil、String、tag 字面量 | supported | nil、false 与 0 共用 f64 表示，见下一行 | core `:tests` |
| `nil?`、Option 在开放类型上的判断 | supported-with-type-evidence | 需要具体类型区分 nil、false 与 0（`E_WASM_NIL_TYPE_EVIDENCE`） | `calcit.core/update-in#updates-existing-nested-leaf` 等排除项 |
| List / Map / Set / Struct / Enum 值与 core 方法 | supported | 线性内存 + bump allocator；`#{}` 字面量不去重 | core `:tests` |
| 已证明的普通方法调用、trait 方法 | supported-with-type-evidence | 接收者需要具体类型（`E_WASM_TRAIT_TYPE_EVIDENCE`、`E_WASM_TRAIT_RESOLUTION`） | `calcit.core/&map:destruct#exposes-pairs-and-folds` 排除项 |
| 作为参数传递的闭包 | supported-with-type-evidence | 调用点可静态特化，参数契约可调用（`E_WASM_CLOSURE_SPECIALIZATION`、`E_WASM_CALL_SPECIALIZATION`） | `calcit.core/&list:filter-pair#filters-key-value-pairs` 排除项 |
| 嵌套 `fn` / `defn` 闭包值、特化闭包中的 `recur` | unsupported | | `calcit.core/&list:apply#preserves-homogeneous-function-result-types` 排除项 |
| `try` | unsupported | 失败只能 trap | `calcit.core/&number:rem#rejects-zero-and-non-safe-integers` 排除项 |
| 局部 `ref`（兼容名 `atom`）、`add-watch!` / `remove-watch!` | unsupported | 局部 Ref 与观察者；已有数值 `defref` 全局不在此排除范围 | `calcit.core/reset!#returns-assigned-scalar` 排除项 |
| 运行时 quote 值、`macroexpand`、`format-to-lisp` 非静态表达式 | unsupported | 宏在编译前展开，不受影响 | `calcit.core/deftrait#requires-head-accepts-bare-and-quoted`、`calcit.core/data-view#classifies-symbol-struct-ref` 排除项 |
| `turn-tag` / `to-tag` 运行时转换 | unsupported | 需要运行时 tag interning（`E_WASM_TAG_CONVERSION`） | `calcit.core/filter-map-kv#preserves-captured-generic-relations` 排除项 |
| `format-cirru-edn`、`try-parse-cirru-edn-as` | supported-with-type-evidence | 需要闭合静态类型与数据形状（`E_WASM_EDN_*`），见 [WASM 编译与验证](../../scripts/wasm-validation.md#cirru-edn-格式化边界) | `scripts/check-parse-boundary.mjs` |
| `decode-map-as`、`parse-cirru-edn-as` | unsupported | | `calcit.core/decode-map-as#decodes-struct-and-option` 排除项 |
| `parse-float` | supported-with-type-evidence | 结果 Option 的 nil 判断需要具体类型（`E_WASM_NIL_TYPE_EVIDENCE`），目前 core 实现无法提供 | `calcit.core/parse-float#wraps-number-or-source-error` 排除项 |
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
&map:fold-kv
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
| `E_WASM_NIL_TYPE_EVIDENCE` | 开放类型上无法区分 nil、false 与 0 | 写出具体类型或使用 Option |
| `E_WASM_TRAIT_TYPE_EVIDENCE` | trait 调用的接收者没有具体类型 | 给接收者写出具体类型 |
| `E_WASM_TRAIT_RESOLUTION` | 已知接收者类型下找不到 trait 实现 | 检查 `impl-traits` 是否覆盖该类型 |
| `E_WASM_TRAIT_CALL` | `&trait-call` 参数不是 trait、方法 tag 与接收者 | 修正调用形式 |
| `E_WASM_CALL_SPECIALIZATION` | 调用点特化需要固定、已知的 arity，或出现递归特化 | 避免可变参数调用或递归高阶调用 |
| `E_WASM_CLOSURE_SPECIALIZATION` | 闭包参数的静态契约不可调用，或闭包逃逸出特化调用 | 直接传入具名函数或固定签名的 `fn` |
| `E_WASM_TAG_CONVERSION` | `turn-tag` / `to-tag` 需要运行时 tag interning | 使用字面量 tag |
| `E_WASM_STRUCT_IDENTITY` | Struct 定义未在编译产物中登记 | 通过具名 `defstruct` 构造 |
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

## 限制

- 不带编号的 `unsupported ... in WASM` 诊断尚未统一格式，不一定包含源位置与缺失的证据种类。
- `wasm-unsupported-procs` 列表只校验排除清单出现过的 Proc，没有 core 测试覆盖的 Proc 靠人工维护。
