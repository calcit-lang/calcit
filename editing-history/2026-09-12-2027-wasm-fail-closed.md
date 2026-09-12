# WASM unsupported 路径改为失败关闭

## 背景

WASM emitter 此前为了维持函数索引，在普通 function lowering 失败时生成固定返回 `0.0` 的 body；匿名或嵌套 closure 也会生成 `nil` placeholder。这会让 unsupported 语义表现为看似成功的业务值。

## 修改

- `init_ns` 中的函数无法提取或 lowering 时，codegen 在写入 artifact 前返回包含 namespace/definition 的错误。
- 依赖 namespace 中已经分配索引但 lowering 失败的函数保留相同 arity 和 table slot，body 改为 WASM `unreachable` trap。
- 通用匿名/嵌套 closure value 不再生成 `nil`，而是返回明确 unsupported；已有 HOF 专用 lowering 保持不变。
- `scripts/test-wasm.sh` 复用现有 Calcit fixtures，验证目标 namespace 失败不写文件，以及实际调用 unsupported dependency 时 trap 而非返回 `0.0`。
- 通用 WASM suite runner 根据模块 import 描述补齐 host function stub，再覆盖已有 math/io 行为，使渐进 suite 能随 host import 集合扩展而继续实例化模块。
- 渐进 suite 在 `set -e` 下显式捕获 codegen exit；目标 lowering 的明确 unsupported 作为待实现切片列出，真正的 build/runtime 失败仍使 suite 失败。

## 测试放置

用户可观察的已支持 WASM 语义继续由 `calcit/test-wasm.cirru` 执行。新增 Node.js 断言仅检查 WebAssembly trap/ABI 边界；目标 lowering 失败复用 `calcit/test-struct.cirru`，不为这项低层边界增加 Rust 语义测试或新的统计 gate。

## 验证

- `CR_WASM_BIN=./target/debug/cr-wasm bash scripts/test-wasm.sh`
- `cargo test --all-targets --quiet`
- `cargo clippy --all-targets -- -D warnings`
- `corepack yarn check-all`
