# WASM 保留已证明的 Tag 类型

## 问题与决策

WASM 的标量 ABI 用数值承载 Tag id，不能仅凭运行时数值区分 Tag 与 Number。
既有 `type-of` lowering 已经优先使用静态证明，并在返回种类前执行原参数一次，
但其标量分类遗漏了 Tag。因此同一段已明确标注 Tag 的程序，在 native / JS
得到 `tag? = true`、`number? = false`、`type-of = :tag`，在 WASM / WASI
却得到错误的 Number 分类。

本次只在现有分类中补齐 `CalcitTypeAnnotation::Tag`，继续复用已经建立的类型
证据与参数求值路径。不从 Tag id 的数字范围猜测种类，不新增运行时类型标记、
公开 API 或诊断，也不改变表层严格类型规则。开放值与 nullable 标量在当前
擦除 ABI 下的限制保留；本次修复不表示已支持返回闭包或全部 Tag 排序路径。

## 共享语义验证

两项回归附在 `calcit.core/number?` 的 `:tests`，覆盖字面量、局部别名、明确
标注 Tag 的函数参数和返回，以及 Number 0/1 的反例。带输出的 producer 分别
经过 `number?`、`tag?` 和 `type-of`，现有统一运行器检查恰好三次输出，避免
只比较后端结果却忽略共同的求值次数错误。未修改任何排除项。

修复前的同源测试在 native / JS 通过，在 WASM / WASI 真实执行时断言失败；
修复后四后端均通过。完整回归继续使用原有测试与运行器：

```bash
node scripts/run-core-tests.mjs --tag tag-kind-evidence --backend native,js,wasm,wasi
node scripts/run-core-tests.mjs --backend native,js,wasm,wasi
bash scripts/test-wasm.sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
yarn check-all
```

跨后端命令通过 `CALCIT_BIN` 固定候选 CLI，WASI 使用实际 Wasmtime；具体运行
结果与版本身份记录在修复 PR，不把构建成功或未执行的检查当作语义通过。
