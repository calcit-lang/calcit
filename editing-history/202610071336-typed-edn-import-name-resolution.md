# typed EDN 的导入类型名解析（#1663）

## 背景与依据

普通 Calcit 构造器能使用 namespace 的 `:as` 别名，但 typed EDN 的 data-shape 推导把 `alias/Def` 当成完整 namespace 路径。因此同一名义类型用完整路径可以解码，用正常 import 别名却在预处理阶段失败。类型名称不能因进入解码边界而采用另一套查找规则。

## 处理方式

- 从既有 namespace 类型解析中抽出 `program::resolve_type_name_in_ns`，由 namespace 类型引用和 data-shape 推导共同调用，保持既有局部定义、`:refer`、core 与限定路径的查找优先级。
- 在解码器预处理阶段规范化类型表达式里的 namespace 别名；后续 data shape、结果类型推导和 backend codegen 使用同一表达式。错误位置保留原始类型表达式的 source location。
- `parse-cirru-edn-as`、`try-parse-cirru-edn-as`、`decode-map-as` 与 `try-decode-map-as` 复用相同路径，不增加按 backend 区分的名称规则或新 CLI。

## 兼容边界

完整类型路径继续有效；别名和 `:refer` 到达原名义声明，不创建新的名义身份。不同 namespace 的同名声明仍独立。未知别名和缺失定义仍使解码器推导失败，不回退 Dynamic。本修复不扩大支持的解码形状，也不处理另外追踪的泛型 Struct 参数证据问题。

## 验证

- `test-edn.main/test-imported-type-names` 的 definition `:tests` 和现有程序入口覆盖别名、`:refer`、完整路径、map 解码及同短名跨 namespace 身份；原 native / 生成 JS runner 保留这些表达式。
- `test-wasm.main/test-struct-edn-identity` 保留原有身份断言，增加别名与完整路径解码结果的相等断言，由现有 WASM 和生成 JS 回放验证。
- `typed_edn_decoder_rejects_unknown_alias_and_missing_definition` 验证两个 EDN parser 入口的编译期错误；不把它宣称为四个解码入口的完整负例矩阵。
- 每次提交仍需以 PR 最新 HEAD 的完整 Actions 与实际 review 验收；既有源码 HEAD 的成功不能代替后续提交验证。
