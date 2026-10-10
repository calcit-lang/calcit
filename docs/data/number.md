---
title: "Number"
summary: "Number 的跨后端规格：表示、算术与非有限值、相等排序哈希、取整、整数参数定义域、转文本与解析"
scope: "core"
kind: "reference"
category: "data"
aliases:
  - "number semantics"
  - "number spec"
  - "remainder"
  - "rem"
  - "safe integer"
  - "NaN"
  - "total order"
  - "rounding"
  - "bitwise"
---
# Number

本页是 Calcit Number 的规格。规则由 core definition `:tests` 覆盖，`node scripts/run-core-tests.mjs` 在 native、生成 JS、WASM 与 WASI 上回放；某个后端暂未符合规则时，在 `scripts/core-tests-exclusions.cirru` 中写明原因；native 尚未符合的规则暂时没有测试，列在末尾的限制中。

## 表示

Number 是 IEEE 754 双精度浮点数：native 中是 `f64`，生成 JS 中是 `Number`，WASM 中是 `f64`。没有独立的整数类型；"整数"指有限且没有小数部分的 Number，"安全整数"指绝对值不超过 `9007199254740991`（2^53 - 1）的整数，在这个范围内整数运算是精确的。

```cirru
assert= 9007199254740992 $ &+ 9007199254740991 1
assert= true $ = 9007199254740992 $ &+ 9007199254740992 1
```

`'Int8`、`'Int32`、`'UInt64`、`'Float32` 等是 source-level refinement，运行时仍是 Number；它们通过 `number->int32` 等受检函数建立边界证明，在 WASI Component 边界映射到 Canonical ABI 的 `i32`、`i64`、`f32`、`f64`，见 [WASM Component 边界](../installation/wasm-component-boundary.md)。

## 算术与非有限值

`&+`、`&-`、`&*`、`&/`、`pow`、`sqrt`、`sin`、`cos` 按 IEEE 754 计算，不因非有限结果报错：除以 0 得到 `inf` 或 `-inf`，`0 / 0` 与 `sqrt -1` 得到 `NaN`，溢出得到 `inf` 或 `-inf`，`-0` 参与运算时保留符号。

```cirru
assert= |inf $ turn-string $ &/ 1 0
assert= |-inf $ turn-string $ &* 1e308 -10
assert= |NaN $ turn-string $ &/ 0 0
assert= |-0 $ turn-string $ &/ -1 $ &/ 1 0
```

对应 `calcit.core/&+#keeps-f64-precision-and-overflows-to-inf` 与 `calcit.core/&/#divides-by-zero-to-nonfinite`。

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

数值比较运算按 IEEE 754 处理（见下一节）；需要排序或稳定的大小关系时使用 `&compare`。

这些规则对应 `calcit.core/&compare#orders-nan-last`、`calcit.core/=#treats-nan-as-equal-value`、`calcit.core/sort#sorts-nan-after-numbers`、`calcit.core/sort#sorts-nan-last-with-compare` 与 `calcit.core/contains?#finds-nan-keys`。

## 比较运算

`&<`、`&>` 及由它们组成的 `<`、`>`、`<=`、`>=` 按 IEEE 754 比较数值：任何一侧是 `NaN` 时结果为 `false`，`-0` 与 `0` 不分大小，`inf` 大于所有有限数。

```cirru
assert= false $ &< (&/ 0 0) 1
assert= false $ &< -0 0
assert= true $ &< 1e308 $ &/ 1 0
```

对应 `calcit.core/&<#compares-nan-as-unordered`。

## 取整与整数判断

- `floor`、`ceil` 按 IEEE 754 向下、向上取整，保留 `-0`。
- `round` 取最接近的整数，恰好一半时远离 0：`2.5 → 3`、`-2.5 → -3`、`-0.5 → -1`；`round -0.4` 得到 `-0`。
- `integer?` 判断有限且没有小数部分：`-0` 与 `1e100` 是整数，`NaN`、`inf` 不是；它不表示安全整数范围。

```cirru
assert= 3 $ round 2.5
assert= -3 $ round -2.5
assert= true $ integer? -0
assert= false $ integer? $ &/ 1 0
```

对应 `calcit.core/round#rounds-ties-away-from-zero` 与 `calcit.core/integer?#integer-alias-boundaries-and-evaluation`。

## 整数参数的定义域

需要整数的参数都有明确的定义域。超出定义域时，native 与生成 JS 抛出可由 `try` 捕获的错误；WASM 没有可恢复的 `try`，对同样的输入 trap。

- **下标与长度**：`&list:nth`、`&list:slice`、`&str:nth` 等的下标是非负安全整数；越界同样报错。
- **`range`**：起点、终点与步长可以是小数；步长不为 0，且方向与区间一致，否则报错。
- **位运算**：`bit-and`、`bit-or`、`bit-xor`、`bit-not`、`bit-shl`、`bit-shr` 的操作数是 i32 范围内的整数，结果是有符号 i32；位移步数只取低 5 位，所以 `bit-shl 1 32` 等于 `1`。
- **`&number:display-by`**：值是非负安全整数，基数为 2、8 或 16，输出带 `0b`、`0o`、`0x` 前缀的精确数字。
- **`&number:rem`**：见下一节。

```cirru
assert= -2147483648 $ bit-shl 1 31
assert= 1 $ bit-shl 1 32
assert= ([] 1 1.25 1.5 1.75) $ range 1 2 0.25
assert= :failed $ try
  do (&list:nth ([] 1 2 3) 1.5) :returned
  fn (_error) :failed
```

对应 `calcit.core/bit-shl#masks-shift-count-to-five-bits`、`calcit.core/&list:nth#rejects-non-integer-index`、`calcit.core/&list:slice#rejects-fractional-bounds`、`calcit.core/range#rejects-zero-step`与 `calcit.core/range#handles-negative-fractional-and-overflow`。

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

## 转文本

`turn-string`、`str`、`to-string` 与 `println` 使用同一套数字文本：

- `inf`、`-inf`、`NaN`、`-0`；
- 十进制展开，不使用指数形式：`1e21` 写作 `1000000000000000000000`，`1e-7` 写作 `0.0000001`；
- 小数部分取能精确往返的最短十进制表示，例如 `0.30000000000000004`。

`&number:format value digits` 保留固定位数的小数，按输入 f64 的精确值舍入，恰好一半时远离 0：`&number:format 2.5 0` 为 `3`，`&number:format 1.005 2` 为 `1.00`（`1.005` 的二进制值略小于 1.005）。

```cirru
assert= |1000000000000000000000 $ turn-string 1e21
assert= |0.0000001 $ turn-string 0.0000001
assert= |-0 $ turn-string -0
assert= |1.235 $ &number:format 1.23456789 3
```

对应 `calcit.core/turn-string#formats-number-boundaries`、`calcit.core/turn-string#shortest-decimal-ties` 与 `calcit.core/str#formats-numbers-like-turn-string`。

## 解析

`parse-float source` 返回 `Result<Number, String>`：

- 接受十进制与指数形式（`1.5`、`1e3`、`-0`），以及 `NaN`、`inf`、`infinity` 及其带符号、大小写变体；
- 不接受首尾空白、十六进制、下划线分隔和不完整的指数，返回 `:err` 并带原文。

```cirru
assert= (%ok 1.5) (parse-float |1.5)
assert= (%err |1e) (parse-float |1e)
```

对应 `calcit.core/parse-float` 的 `wraps-number-or-source-error`、`reports-invalid-number-source` 与 `parses-nonfinite-spellings`。

## 限制

- 生成 JS 中 `round` 恰好一半时向正无穷取整，WASM 取偶数。
- native 的 `&number:format` 恰好一半时取偶数，WASM 不支持 `&number:format`。
- 生成 JS 接受小数的 `&list:slice` 边界；位运算的越界与小数操作数在 native 饱和或报错、在 JS 回绕或截断、在 WASM trap 或截断。
- native 的 `&number:display-by` 经过 i32 转换，超出 i32 的值与负数不能正确输出；生成 JS 接受负数与小数。
- WASM 不支持 `parse-float`；对定义域之外的输入只 trap，不提供错误消息。
