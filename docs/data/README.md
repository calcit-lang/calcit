# 核心 API 契约基线

`core-api-0.28-scope.edn` 是已审阅的转换、解析、副作用、谓词与集合首选入口的范围与失败语义；`core-api-0.28-baseline.edn` 用现有 `query def/type` 导出对应的声明证据。两者都是原生 **Cirru EDN**，不是 Snapshot，也不参与运行时派发。

本批是 **0.28.0 candidate 的部分基线**，不是整个 core 已冻结的声明。现已纳入公开数学函数、`fs:path` 首选构造、ToString/Len/Add/Eq 约束及已审阅的集合/谓词方法，不因 internal tag 漏掉应用入口。其余公开边界/trait、兼容名移除版本、实际 backend 支持矩阵与集中迁移例外仍未完成。#1568 的策略基础已关闭，剩余验收由 [#1458](https://github.com/calcit-lang/calcit/issues/1458) 与 milestone 收尾跟进；issue 状态不改变部分基线的覆盖范围。失败描述是人工审阅的语义记录，不是脚本自动证明；行为仍以已有 Calcit `:tests`、严格负例和 host/backend 回归为准。

标量转换另外记录 String、Number、Bool、Tag、Symbol、Nil 的首选 `.to-string` 实际调用契约，返回值均须保持 String，不能因内部实现声明较宽而丢失派发后的证据。Debug/Show 的公开 trait 声明也纳入范围，避免将调试显示、面向人的显示和值转换误合并；不因此承诺所有类型都实现这三种能力。方法实现路径仍仅是 provenance，编译器可以重构内部 helper 而不改用户调用契约。

`to-string` 的 `wasm-scalar-method-contract` 附带测试使用普通 `.to-string`，同一断言 AST 在 native、生成 JS 和实际 WASM 执行，覆盖 String、Number、Bool、Tag、Nil 的文本结果及 String 返回类型。WASM fixture 另外从宿主传入运行时 Number，核对普通方法返回的 UTF-8 文本，包括小数、正负零、非有限值和 f64 边界；不改成 native call 或内部 primitive 来绕过方法 lowering。Symbol 仍只计入已经验证的 native/JS 范围，不能因其他标量通过而宣称 WASM 支持 Symbol。

## 显式转换边界

显式箭头转换的候选声明覆盖 `number->int*/uint*/float*` 与 `js-nullish->option`。前者保留 `Number -> Result<Refinement,String>`，成功值仍是原 Number，精确表示和范围验证不是自动舍入；后者保留 `JsNullish<T> -> Option<T>` 的同一 T 与 `:js-ffi` feature，仅包装空值边界，不能充当 payload decoder。两者职责不同，不因同用箭头拼写而合并错误模型或新增 `to-/as-/into-` 别名。此处记录既有 schema，不新增 backend 支持，也不冻结未经证明的动态类型。

## 已验证的后端范围

以下是当前候选中几个关键边界的**测试证据索引**，不是所有 core API 的支持矩阵。声明 schema、查询的 `proven`、成功生成产物、实际执行成功是不同层次：`proven` 只证明当前接收者上的静态调用契约，不自动承诺某个 backend 或宿主支持。未列出的入口仍须查看对应实现与测试，不能从同族名字推断支持。

| 边界 | native / 生成 JS 的证据 | WASM / WASI 的证据与限制 |
| --- | --- | --- |
| `integer?/.integer?` | `check-numeric-predicate.mjs` 完整重放既有四个 Calcit `:tests`，检查有限整数和实参求值次数 | 同四份 AST 在 core WASM 执行；设置 `WASMTIME_CLI` 时也运行真实 WASI 0.3 Component |
| `number->int*/uint*/float*` | 同脚本完整重放十个 `checks-boundaries-and-type`；保留成功值、失败分支及 `Result<Refinement,String>` 断言 | 此脚本不宣称转换测试的 WASM/WASI 覆盖。Int8 测试包含当前 WASM 不支持的 throwing `parse-cirru-edn-as`；不删掉该断言，也不把谓词测试通过当作转换测试通过 |
| 标量 `.to-string` | `check-typed-string-conversions.mjs` 重放普通方法断言，并验证 ToString 约束拒绝未证明的输入 | String、Number、Bool、Tag、Nil 的受支持 AST 和运行时 Number 文本在实际 core WASM 执行；Symbol 不计入该范围，开放泛型导出仍拒绝 |
| String `.parse-float/.parse-json/.parse-cirru/.parse-cirru-edn/.parse-cirru-list` | `check-parse-boundary.mjs` 重放同一份 Result 方法及解析边界测试，包括错误文本与嵌套 payload 拒绝 | 不宣称通用 parser 的 WASM 支持；不能因为返回 Result 就假定可 lowering |
| 闭合 `try-parse-cirru-edn-as` 与 `format-cirru-edn` | 闭合 decoder 由解析边界测试验证；不是开放 JSON/Cirru 值的隐式强转 | 递归容器和 nominal 数据的受支持 shape、容量限制和拒绝行为见 [WASM 验证说明](../../scripts/wasm-validation.md#cirru-edn-格式化边界)。Manifest 文件业务由现有脚本分别验证 Preview 1 与默认 Component，不能将整份历史 fixture 的能力移植为默认 Component 承诺 |
| `FsPath .read-text/.write-text!` | `test-wasi-manifest-business.sh` 使用同一份 Manifest 输入、输出和 Result 分支在 native 与 Node JS 执行 | 同脚本验证显式 Preview 1；`test-wasi-manifest-component.sh` 单独验证默认 WASI 0.3 Component。`.read-dir/.walk-dir` 不因文件读写通过就获得默认 Component 支持 |

上表引用的是仓库源码回归，不是新安装包或消费者兼容性证明。跨仓库升级仍要使用匹配的已发布精确版本，并执行真实项目回归。`js-nullish->option` 的 `:js-ffi` 和同一 payload 泛型保留在声明基线中；它仍是 JS 空值包装边界，不属于通用 WASM decoder。

维护者可在已经构建本仓库 binary 和 JS runtime 后复跑现有数字检查，不新增用户命令：

```bash
node scripts/check-numeric-predicate.mjs
WASMTIME_CLI=/absolute/path/to/wasmtime node scripts/check-numeric-predicate.mjs
```

第二条使用维护者提供的真实 Wasmtime；CI 在 Component job 中传入固定宿主。未设置变量时脚本只执行 native、生成 JS 与 core WASM，日志明确区分十个转换测试和四个谓词测试的覆盖范围。该索引不取代后续完整 backend 矩阵，也不放行候选基线中尚未完成的移除版本与集中迁移验收。

## 声明导出与历史比较

普通用户仍使用已安装版本的 `calcit query def/context/type --format edn`。以下是仓库开发步骤，不新增用户 CLI 入口：

```bash
cargo build --bin calcit
yarn compile
node scripts/core-api-contract.mjs --export
node --test scripts/core-api-contract.test.mjs
node scripts/core-api-contract.mjs
```

导出只写 stdout，先审阅再更新基线；不是自动接受签名变化的命令。Node 子进程显式选择 JSON 仅作为工具桥接，持久化产物保留 symbol、tag 和 `quote` 包裹的原始 schema。泛型与 `where`、receiver 参数、返回类型、名义类型声明和 optional/rest arity 都不以展示字符串代替。`read-dir` 的 Bool schema 不代表 recursive 参数必填，以已有运行时 arity 为准。

方法先在明确 receiver 上查询，必须 `proven` 且不能是已识别的兼容入口，再追溯原始声明 schema。Number 实例用于派发检查，不把它误写为方法的唯一可用类型；泛型关系仍保存在 schema。尚为 `open/ambiguous` 的方法不会自动加入。`FfiTask/FfiResponse` 的 raw Dynamic 是现有宿主边界，不意味着业务层可以绕过类型约束。

`method-contracts` 另外保存当前接收者上的 `call-types` 原生 quote：例如 List/Map 的 `.get` 实现声明允许开放值，但编译器已经证明具体调用返回 `Option<Number>`。即使实现的宽 schema 没变，具体调用结果或 callback 类型关系变差也不能绕过检查。这些参数不含隐式接收者；`methods` 中的 `runtime-arity` 则是实现函数的 arity，包含接收者，两者不能混用。共享接收者查询只执行一次，不重复运行同一分析。

Option `.map` 目前查询仍为 open，解析开放 JSON/Cirru EDN 等边界仍需要单独审阅，不能靠本清单或目标类型把它们包装成 proven。候选只记录真实证据，不把推导漏洞固化为永久动态契约。0.29 的断言/调用证明工作仍由 #1538 拥有。

解析家族的 `try-parse-json`、`try-parse-cirru`、`try-parse-cirru-edn` 与 `try-parse-cirru-list` 函数声明也纳入候选：输入为 String，失败为 String，成功值分别为开放数据、CirruQuote、开放数据与开放 List。记录声明不等于证明开放 payload，JSON/Cirru EDN/List 的方法契约仍不进入要求 `proven` 的方法冻结范围。需要业务类型时使用现有闭合 decoder，而不是插入 unsafe 或假定目标类型。

`scripts/check-parse-boundary.mjs` 在 native 与生成 JS 执行同一份 definition `result-method-contract` AST，覆盖普通 String `.parse-float/.parse-json/.parse-cirru/.parse-cirru-edn/.parse-cirru-list` 的成功、失败和 Result 类型断言，并继续执行闭合 decoder 的嵌套成功/拒绝用例。此批不宣称这些通用文本 parser 的 WASM 支持；已有闭合 Cirru EDN WASM decoder 的支持与限制见 [WASM 验证说明](../../scripts/wasm-validation.md)。

CI 检查当前源码与基线，并对 PR 的 merge-base 基线重新比较；push 比较父提交。只改范围或重生成基线不能放行既有名字、schema、失败记录或 receiver 的变化。合法新增允许；函数体、局部参数名和方法内部实现路径不冻结。当前不提供 breaking-change 豁免，#1568 规划的集中迁移例外仍由 #1458 跟进，须实现同一 PR 的映射与语义验证，不能手动跳过检查。

这里的测试放在 Node，是为了验证导出格式、Git 基线防绕过和契约比较这些维护工具边界，不重复添加 Rust 语言语义测试，也不新增动态类型统计 analyzer。失败描述的变化可由历史比较发现，但实现是否违反失败语义仍须由共享语义测试证明；不能把“文字没有变化”当作行为已经验证。
