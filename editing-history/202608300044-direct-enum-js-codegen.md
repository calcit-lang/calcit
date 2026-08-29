# Direct named data constructors in JS codegen

## Context / 背景

Direct named enum calls such as `schema/Op :effect/ping` were lowered with an
embedded compiler-only definition value. The native evaluator accepted that
value, but JS codegen panicked because no runtime module reference remained.

直接调用具名枚举（例如 `schema/Op :effect/ping`）时，预处理曾把编译期类型定义嵌入
运行时代码。Native evaluator 可以处理该值，但 JS codegen 因缺少模块引用而 panic。

## Change / 修改

- Preserve the `(namespace, definition)` path from a preprocessed `Import`
  when lowering direct Struct and Enum constructor calls.
- Emit a regular codegen error for leaked compiler-only values instead of
  reaching an `unreachable!` panic.
- Cover import-path preservation and the non-panicking diagnostic with unit
  tests.

- 直接 Struct/Enum 构造器降级时保留预处理后 `Import` 的命名空间与定义路径。
- 编译期专用值意外进入 codegen 时返回普通诊断，不再触发 `unreachable!`。
- 增加引用路径保留及无 panic 错误路径的单元测试。

## Verification / 验证

- Targeted Rust tests pass.
- A Calcium Workflow copy using direct cross-namespace `schema/Op` construction
  completes full server-side JS emission with the patched release compiler.

- Rust 定向测试通过。
- 使用跨命名空间 `schema/Op` 直接构造的 Calcium Workflow 副本，已用修复后的
  release 编译器完整完成 server JS 输出。
