# String 到 Tag/Symbol 的类型化入口

## 决策依据

`turn-symbol` 与 `turn-tag` 的历史运行时输入比应用层想承诺的类型更宽。此轮新增 `to-symbol: String -> Symbol` 与 `to-tag: String -> Tag`，作为转换命名方案的可审查候选；不复用泛型 `T -> Target` schema，也不把 `str`、`turn-string`、Show/Debug 混为一类。旧入口保留给 core 宏和现有消费者，不批量按词形替换。

## 兼容与目标边界

native 与生成 JS 均保留旧实现的运行时行为，但新入口的静态契约只接受 String。WASM 目前使用静态 Tag 编号，缺少运行时 Tag/Symbol intern。原 `turn-tag` lowering 将 String 原样返回，值与 Tag 字面量不相等；现改为明确的 `E_WASM_TAG_CONVERSION` 不支持诊断，并在可达依赖处产生 trap，避免悄悄返回错误类型。后续若要支持动态转换，应设计可跨 core WASM/WASI 的 intern 方案，而非局部替换标签编号。

## 验证

Calcit core definition `:tests` 覆盖两个入口；严格负例覆盖 Number、List、Tag、nil；native eval 与生成 JS 验证返回名义类型和值；WASM 回归验证转换依赖显式不支持，现有 WASM 套件仍通过。后续 guarded fix 只可针对已证明 String 输入，且需分别核对旧/新调用的求值与失败语义；Respo 消费者升级和完整 #1456 验收另行处理。
