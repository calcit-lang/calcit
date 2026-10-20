# Option 擦回 nil 的提示按宿主参数位置豁免

关联 #2124 剩余顺序第 2 项。原实现在整个 `:js-ffi` 定义内不报告 `.unwrap-or … nil`、
`option:unwrap-or … nil` 和 `match … (:none) nil`，使适配函数里的业务代码也一并豁免。

现在预处理为每个列表形式记录一帧，标明它的源头是否把参数直接交给 JS 宿主：`js/…` 全局、
`.!method` / `.?!method` 原生方法、external-object 方法与字段写入、`new`、`js-array`、
`js-object`、`js-set`、`aset`，以及目标是宿主属性（`.-field` 或 `js/…`）的 `set!`。
external-object 方法的源头在预处理前看不出来，前缀形式在接收者类型确定后、后缀形式在
改写为 `ExternalInvoke` 时把当前帧标为宿主。擦除形式只有作为这类调用的直接参数时才不报告；
先绑定局部、`set!` 到局部、被 `nil?`/`if` 等消费、或经宏改写后的位置仍报告。返回类型声明为 `JsNullish`
的函数维持原有豁免。提示仍是非阻塞 Notes，ordinary/strict 结论不变。

尚未覆盖：形参声明为 `JsNullish` 的普通 Calcit 函数调用位置；需要先解析被调用者 schema，
留待后续切片。

验证：`cargo test --bin calcit -- option_erased_to_nil unwrap_or_nil`，fixture
`calcit/type-fail/option-nil-host-position.cirru`。
