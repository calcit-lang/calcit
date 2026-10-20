# Option 擦回 nil 的提示豁免 `JsNullish` 形参位置

关联 #2124。#2215 把 `W_OPTION_ERASED_TO_NIL` 的豁免收窄到 JS 宿主调用的直接参数，
但普通 Calcit 函数的形参声明为 `JsNullish<T>` 时，传入的 nil 同样是声明过的合同，仍会报告。

现在预处理在处理调用参数时记录该参数对应的被调用者形参：形参为 `JsNullish<T>`
（`Nullable(_, JsHost)`）时，这个参数位置与宿主调用的直接参数一样不报告。具名函数调用
（`preprocess_known_function_call`）与局部或间接可调用值的调用都覆盖；展开参数之后的位置
不再对应形参，不豁免。普通形参、先绑定局部或被业务代码消费的 nil 仍报告。

strict 下 `.unwrap-or (get m :a) nil` 本身无法证明 `JsNullish<T>` 形参，所以 fixture 用
已证明 payload 的 `match … (:none) nil` 形式覆盖这条路径。

验证：`cargo test --bin calcit -- option_erased_to_nil`，fixture
`calcit/type-fail/option-nil-host-position.cirru` 的 `to-nullish-param` / `to-plain-param`。
