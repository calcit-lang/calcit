---
title: "Number"
summary: "Number 的跨后端语义：相等、排序与哈希，以及取余 rem 的定义域、结果与错误"
scope: "core"
kind: "reference"
category: "data"
aliases:
  - "number semantics"
  - "remainder"
  - "rem"
  - "safe integer"
  - "NaN"
  - "total order"
---
# Number

Calcit 的 Number 在 native 中是 `f64`，在生成的 JavaScript 中是 `Number`，在 WASM 中是 `f64`。本页记录已经在三个后端统一、由同一组 core `:tests` 验证的数值运算语义。

## 相等、排序与哈希

`=`、`&compare`、`sort`、Set 成员与 Map 键在 native、生成 JS 与 WASM 中使用同一组数值规则，保证相等、排序与哈希互相一致：

- **相等**：`NaN` 等于 `NaN`，`0` 等于 `-0`；其余数字按数值相等。
- **排序**：`&compare` 与默认比较的 `sort` 使用全序：`-inf` 最小，`-0` 与 `0` 相等，`inf` 之后是 `NaN`，所有 `NaN` 彼此相等。
- **哈希**：相等的数字哈希相同，所以 `NaN` 可以作为 Set 元素或 Map 键被找回，`0` 与 `-0` 是同一个键。

```cirru
let
    not-a-number $ sqrt -1
  assert= true $ = not-a-number not-a-number
  assert= true $ = 0 -0
  assert= 1 $ &compare not-a-number $ &/ 1 0
  assert= ([] -1 0 3 not-a-number) $ sort $ [] 3 not-a-number 0 -1
  assert= true $ contains? (#{} not-a-number) not-a-number
```

数值比较运算 `<`、`>`、`<=`、`>=` 仍按 IEEE 754 处理：任何一侧是 `NaN` 时结果都是 `false`。需要排序或稳定的大小关系时使用 `&compare`。

这些规则对应 `calcit.core/&compare#orders-nan-last`、`calcit.core/=#treats-nan-as-equal-value`、`calcit.core/sort#sorts-nan-after-numbers`、`calcit.core/sort#sorts-nan-last-with-compare` 与 `calcit.core/contains?#finds-nan-keys`。

## 取余 `rem`

`&number:rem a b` 与 Number 方法 `a .rem b` 是同一个运算，native、生成 JS、core WASM 与 WASI Component 使用同一契约：

- **定义域**：两个操作数都是安全整数，即有限、恰好没有小数部分，且绝对值不超过 `9007199254740991`；除数不为 0（包括 `-0`）。
- **结果**：截断取余，`a - b * trunc(a / b)`。结果的符号跟随被除数 `a`，与除数的符号无关；结果为 0 时总是 `0`，不返回 `-0`。
- **精度**：定义域内的结果是精确值，不经过 32 位整数转换，超出 i32 的值不会被截断。
- **定义域之外**：native 与 JS 抛出可由 `try` 捕获的错误，两者的消息相同；WASM 没有可恢复的 `try`，对同样的输入 trap。

```cirru
assert= -1 $ &number:rem -7 3
assert= 1 $ .rem 7 -3
assert= -1 $ .rem -7 -3
assert= 0 $ &number:rem -2147483648 -1
assert= 4 $ &number:rem 4294967296 7
assert= 1 $ &number:rem 9007199254740991 10
```

错误消息有两种。零除数为 `&number:rem divisor must not be zero`；小数、NaN、Infinity 或超出安全整数范围为 `&number:rem requires safe integers, but received: <a> <b>`，其中两个操作数按 Calcit 的数值文本输出（例如 `inf`、`NaN`，大数展开为十进制而不是指数形式）：

```cirru
assert= "|&number:rem divisor must not be zero" $ try (&number:rem 1 0)
  fn (error) error
assert= "|&number:rem requires safe integers, but received: 5.5 2" $ try (&number:rem 5.5 2)
  fn (error) error
assert= "|&number:rem requires safe integers, but received: 9007199254740992 3" $ try (&number:rem 9007199254740992 3)
  fn (error) error
assert= "|&number:rem requires safe integers, but received: inf 2" $ try
  &number:rem (&/ 1 0) 2
  fn (error) error
```

需要对小数取余时，显式写出舍入方式，例如基于 `floor` 的 `&- a $ &* b $ floor (&/ a b)`；结果符号与舍入方式由业务决定。

这些规则对应 `calcit.core/&number:rem` 的两个 `:tests`：`calculates-truncated-remainder` 覆盖正负数、0、超出 i32 与安全整数边界，`rejects-zero-and-non-safe-integers` 覆盖零除数、小数、超出安全整数和非有限值。可用 `calcit query def 'calcit.core/&number:rem' --format edn` 查看定义与测试。0.29 之前各后端的差异与迁移说明见[升级说明](../run/upgrade.md#取余的跨后端语义)。

## 限制

- 本页只覆盖相等、排序、哈希与 `rem`；转文本、位运算、下标等数值语义仍以各 API 的定义与测试为准。
- WASM 对定义域之外的输入只 trap，不提供错误消息。
