# 2026-0225-0002 eval 模式下的语言行为要点

通过对 guidebook 文档代码块进行 eval 验证，发现以下 Calcit 运行时与预处理器的关键行为。

## `list-match` 仅适用于 List，不接受 Tuple

`list-match` 的展开宏内部调用 `&list:slice`，该函数在 `src/builtins/lists.rs` 中要求输入为 `:list` 类型。
若传入 `::` 创建的 tuple，预处理阶段报 `Proc &list:slice arg 1 expects type :list, but got :tuple`。

正确用法只针对 list，且分支模式是 **`(head tail)`**，`tail` 是剩余元素组成的 list（不是按位置展开多个变量）：

```cirru
list-match ([] :point 10 20)
  () |Empty
  (h tl) ([] h tl)
; => ([] :point ([] 10 20))
```

## `.-field` 属性访问是 JS codegen 专用

`.-x p` 这类属性访问在 `src/codegen/emit_js.rs` 中实现，Rust 解释器（`src/runner/`）没有对应路径。
eval 模式下报：`method kind access (.-x) is only available in JS codegen, not supported in Rust runtime`.

替代方式：对 struct record 字段用 `(:field-name record)` 或 `get record :field-name`。

## 数值/字符串字面量在 fn 体内裸行的解析问题

`fn`/`defn` 体内，一个字面量（数值 `3.14159` 或字符串 `|demo`）若单独占一行，Cirru 解析器会把该行当作以字面量为 head 的调用列表，从而报 `unknown head 3.14159` 或 `unknown head |demo`。

修复方式：加 `, ` 前缀强制作为数据表达式（expression terminator 后接值）：

```cirru
defn get-pi () :number
  , 3.14159
```

## `tag-match` 要求 defenum variant 的 payload 数量严格匹配

`:ok` 无 payload 类型时，`%:: E :ok 42` 传 1 个值报：`enum variant ok expects 0 payload(s), but received: 1`。
需在 `defenum` 定义中声明对应类型：`defenum Result (:ok :number) (:err :string)`。

该检查在 `src/runner/` 中 `%::` 调用路径执行（运行时，非预处理）。

## 变量名遮蔽 calcit.core 时触发 warning 并中止 eval

预处理器对以下 core 名字的遮蔽一律报 warning，而 eval 模式将任何 warning 视为错误：

- `first` → 遮蔽 `calcit.core/first`
- `rest` → 遮蔽 `calcit.core/rest`
- 宏参数名 `cond` → 遮蔽 `calcit.core/cond`（在 `defmacro when-not (cond & body)` 中出现）

命名建议：用 `n`, `tl`, `h`, `item` 等无冲突名。

## `let (x 1)` 单括号写法是无效语法

Cirru `let` 绑定在 `src/runner/` 中要求双层 list `((name value) ...)` 形式。
`let (x 1)` 单括号会得到 `expects pairs in list for let, got: ([] 'x 1)` 错误。

有效形式：

```cirru
; 多行缩进
let
    x 1
  x

; 单行（需双括号）
let ((x 1)) x
```

## deftrait/defimpl/defenum/defstruct 在 let 绑定中可正常执行

所有类型定义表达式都是普通可求值的形式（返回定义值），可放入 let 绑定：

```cirru
let
    MyFoo $ deftrait MyFoo
      :foo $ :: :fn ('T) ('T) :string
    MyFooImpl $ defimpl MyFooImpl MyFoo
      :foo $ fn (p) (str-spaced |foo (:name p))
    Person0 $ defstruct Person (:name :string)
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  .foo p
```

这使得在 eval/check-md snippet 中也能完整测试 trait dispatch，无需顶层 `ns/def` 定义。
`def` 是顶层定义专用，eval 模式下不可用，需替换为 `let (Name $ ...)` 形式。

## 多行类型注解 tuple 在运行时报错

`:: :fn ([] :number) :string` 的多行缩进形式中，最后 `:string` 独占一行，被运行时当作 field access 调用，导致错误。
紧凑单行形式 `(:: :fn ([] :number) :string)` 可运行。
