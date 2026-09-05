# Preserve positional nominal generic parameters / 保留具名类型泛型声明顺序

Issue: #877. Base: dd8138aa2f41d213544453fbbf5ca618ea1996c7.

- Static Struct/Enum parsing and runtime definition constructors sorted explicit
  generic names, changing Result<T,E> into positional [E,T]. Keep declaration
  order with stable first-occurrence deduplication; field/variant sorting stays.
- 静态解析和运行时构造原先按字母排序泛型参数，破坏 Result 成功值/错误的类型
  对应关系。保留原声明顺序及去重语义，不改变字段与变体排序。
- Added static/runtime nominal metadata agreement (including a non-adjacent
  duplicate), Result ok/err type-checking, and native/JS integration assertions.
- `parse-float |41` followed by ok arithmetic now evaluates to 42. Rust tests:
  712 library + 300 CLI + 23 integration pass; native suite passes.
- Initial generated-JS execution lacked the repository-local @calcit/procs
  self-link; use the existing yarn procs-link setup before repeating it.
- Full yarn check-all (including JS, Agent interface 18/18, core tests 237,
  WASM and lowering checks) and all-target Clippy pass. Added negative Result
  payload misuse checks after the full run; rerun the focused Rust test.
- Calcium local regression using this unreleased debug compiler: restored the
  canonical Result<Value,Error> in patch validation and four message decoder
  schemas previously written against sorted [E,T]. Normal client/server
  preprocessing, 21 tests, JS codegen and Session/User JS suite pass. Published
  dependency pins and the globally installed compiler remain unchanged.
- Calcium 本地回归恢复标准 Result 顺序后通过；未发布的本地编译器仅用于验证，
  不替代正式版本依赖。草稿 PR #47 仍需完成其他严格边界验收。
- Latest-head review and Actions remain required. No release/version changes
  or merge-completion claim.
