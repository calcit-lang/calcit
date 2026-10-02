# 调用泛型与独立退出证据

本轮围绕 #1700、#1702、#1707、#1708 修复真实严格检查中的证据丢失，保持 #1553 的方向：共享类型关系负责证明，改写和后端只保留已经证明的语义。没有新增 analyzer、公开命令、诊断编号或自动 cast。

## 泛型所有权与回调

每次调用只实例化 callee 自己声明的泛型；调用方的词法泛型保持 rigid。回调的独立泛型在当前调用内实例化，固定参数、rest、返回值和 trait bounds 使用同一份证明。嵌套 Fn 的泛型会遮蔽外层同名变量，捕获变量不会被误当作新泛型。证明失败不能留下部分绑定；实际 Dynamic 输入不能借另一参数的具体类型获得虚假精度。

空集合没有元素证据，但只在对应集合家族中参与上下文推导。开放变量、非空集合和错误集合家族不能获得同样待遇。fold 的初始值和 callback 必须独立满足参数关系，不用声明的返回值反向证明输入。

## 退出与分支合并

受检尾递归保持当前函数的参数实例化，不能产生新的泛型绑定。独立值退出提供返回类型；只有递归循环而没有独立值退出时，不能凭声明建立具体返回证明。

真正的 `raise` 退出不产生返回值。共享退出检查穿过 lexical let，并要求条件或 match 的所有相关分支都抛错，才证明整体必然抛错。该证明只消除无法发生的返回违约，不把异常表达式推成某个具体值类型，也不按普通符号名字猜测抛错行为。

nominal Enum 分支合并读取实际构造器的 variant payload 契约。没有被该 variant 使用的泛型槽不提供证据；实际 payload 中的 Dynamic、开放 Enum 值和无构造器证据的表达式仍参与保守合并。这条规则适用于普通自定义 enum，而非只为 Result 增加特例。原 Diary `prepare-client-patch` 的成功 payload 因此保留，原 Snapshot 不需要添加 cast 或放宽 schema。

## 定义值与公开契约

StructDef/EnumDef 值与实例类型保持区分。已知非 nullable nominal enum 的定义查询保留 EnumDef 证据；匿名或 nullable 输入不被当作已知 nominal 定义。按定义名字查询类型时展示实例与方法契约，表达式查询仍展示实际定义值类型。

`get` 保留开放 Struct 的 tag/string/symbol key 行为，在已有运行时检查通过后显式转成 Tag；缺失字段和非法 key 仍返回 None。`str-spaced` 保留异质值与 nil 的格式化行为，先由 `&str` 生成 String，再调用 String-only helper；零参数调用在生成产物前拒绝。

## 验证与迁移边界

用户可观察语义放在 core definition 的 `:tests`，统一运行器回放 native/JS 正例与原有 native/JS/WASM/WASI 负例。Rust 测试覆盖泛型绑定、slot participation、scope 和协议的不变量；WASM 支持的 named recur 路径与未支持的局部闭包 recur 分开验收。

strict workflow 正例必须无顶层诊断并产生可恢复计划。旧项目负例必须定位实际调用或 producer：安全改名不会让 Dynamic 成为 Enum，也不会修复错误的索引类型。测试不再期待无关 core 的旧错误，而要求准确的项目诊断；安全迁移已应用仍不能冒充完整验证通过。

提交前重新运行完整 Cargo、Agent interface、API 基线、统一跨后端回放、core tests、格式与 Clippy，保留真实项目和原负例。PR 的最新 HEAD CI/review、精确 main SHA workflows 与 milestone 发布门禁仍需独立完成，本记录不代替那些验收。

全量 definition tests 暴露的 `&list:apply` 函数列表证据丢失已通过通用推导修复：将实际调用的集合成员契约传给 literal callback 的参数，输出仍独立来自函数体；函数集合采用参数逆变的共同 callable 契约，互不兼容的返回值仍丢失具体证据。集合展开不走 literal 快捷路径，异步 pending 输入仍经过统一检查。原方法调用与原测试代码保持不变，并加入现有 native/JS 回放器，没有新增检查入口。

该候选的完整 Cargo（包括本机 HTTP/WASI 回归）、全部 469 条 definition tests、Agent interface、API 基线、统一严格回放与 Clippy 已通过。另补函数列表混合返回值与开放 callback 输出的严格修复负例，要求失败且 Snapshot 不变；普通兼容模式的开放边界不冒充严格证明。最终 PR/rebase 的 HEAD 仍须重新验证。

## 独立 review 补充

PR review 指出两个共享证据入口遗漏：仅声明返回值的 metadata hint 不应清空从原始 body 收集的参数类型；proc 的 definition alias 与 runtime value 路径也必须声明自己的泛型。前者只在实际选中的 hint 显式包含 `:args` 时覆盖参数，避免另一个 hint 的字段影响当前签名；后者统一使用已有 `from_proc_parts`，不增加新的 alias 特例。

原始 runtime form 的参数收集和 runtime proc 的泛型归属属于预处理之外的内部 metadata invariant，使用 Rust 精确回归；用户可观察 callback 行为仍由既有 Calcit definition tests 与 native/JS 回放覆盖。补丁须重新构建和验证，上一 HEAD 的通过不代替本轮验收。

新增 proc definition alias 的 Calcit `.map` 附带测试后，native 通过，但生成 JS 报 `$procs is not defined`：Proc definition 分支假定定义名就是 runtime export 名，且假定命名空间已经导入 `$procs`。该分支改为复用实际 codegen value 的普通表达式生成与 namespace-aware proc 映射，并导出 alias 绑定。保留方法调用和原函数身份，不添加额外 runtime import 或 wrapper。既有 source-alias 运行器跨命名空间回放该测试，同时验证真实 alias 可被导入。
