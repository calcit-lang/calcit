# 旧的 `defimpl ImplName Trait` 与顶层 `impl-traits` 改为报错（#2727 第 2b 步）

## 决策

- Jon 在 2026-11-12 确定：0.43.0 起旧写法直接报错，不保留过渡期。生态里的 4 个库要在发版前完成迁移。
- 顶层由 `(:impls …)` 自动挂载；函数体内的局部 struct/enum 继续用 `impl-traits`，其中的 `defimpl` 同样以类型开头。

## 实现

- `E_LEGACY_DEFIMPL_NAME`：在 `process_macro_source` 展开 core `defimpl` 前检查。第一个参数如果是局部绑定，类型必须是 StructDef/EnumDef（Dynamic 放行）；如果没有类型，例如旧的 `Impl $ defimpl Impl Tr` 读到自己的绑定，则报错。顶层名字由 `program::names_nominal_type_definition` 判断，它沿 `def X Y` 的别名链查找，最多 8 层。
- `E_LEGACY_IMPL_TRAITS`：处理 `impl-traits` 调用时，如果所在定义的源码本身就是 `impl-traits` 或 `def X (impl-traits …)`，则报错。手写的限定形式 `calcit.core/impl-traits` 同样报错；编译器从 `(:impls …)` 生成的代码记录在 `WRITTEN_CODE` 里，按记录排除。spread 参数的调用也会检查。
- bundled core 的 ns 不做这两项检查。
- JS 输出顺序：类型 `X` 的 `(:impls …)` 改写成引用实现之后，实现方法体又引用 `X`，形成环。`sort_compiled_defs_by_deps` 对 `defimpl` 定义（按源码开头或 `Impl` schema 识别）丢掉指向自身依赖环内的懒引用边，保证实现先于挂载它的类型输出。只对实现剪边：普通函数可能在别的值初始化时被调用，它读取的值必须保持在前。

## 迁移

- 仓库内的 snapshot、fixtures 和文档代码块全部改写成新形状。
- 原来用同一个基类型挂不同实现的顶层别名，拆成各自独立的类型，或改由函数内 `impl-traits` 构造。test-traits 里需要保持同一名义类型的回归测试（ExplicitTrait*、OpenContains*）改为后者，仍以 `OpenContainsBase` 为基类型，由 `explicit-trait-false`、`open-contains-false` 等函数在函数体内挂载，挂载的实现和顺序不变。不要让基类型用 `(:impls …)` 带上一个实现，再在局部挂上同一 trait 的另一个实现：运行时 `&trait-call` 会报 `E_DUPLICATE_TRAIT_IMPL`。`explicit-nominal-contains?` 的参数不再静态带有实现，`&trait-call` 的返回值无法证明，因此改成 `if … true false`。构造函数的返回类型是不带实现的基类型，依赖静态实现的测试(`:where` 约束、core `contains?` 走 trait 方法)要在 `let` 里直接 `impl-traits` 构造值。
- 兼容边界：WASM 后端不编译函数体内的 `impl-traits`（报 `E_WASM_NIL_TYPE_EVIDENCE` / `E_WASM_TRAIT_RESOLUTION`），所以改用局部挂载的 `impl-origin-portable` 测试在 WASM 上无法通过；`node scripts/run-core-tests.mjs --snapshot calcit/test-traits.cirru --tag impl-origin-portable --backend native,wasm` 可复现。Jon 在 2026-11-12 决定这组测试 WASM/WASI 不再运行：受影响的 42 个测试 ID 逐个写入 `scripts/core-tests-exclusions.cirru` 的 `:wasm` 和 `:wasi`，原因以 `unsupported` 开头并注明实际诊断；native/JS 继续完整运行，同标签下其余 11 个测试仍在 WASM 上运行。

## 验证

```bash
cargo test --bin calcit -- defimpl_rejects top_level_impl_traits
node scripts/run-core-tests.mjs --snapshot calcit/test-traits.cirru --backend native,js
calcit calcit/test.cirru --no-proof-audit
```
