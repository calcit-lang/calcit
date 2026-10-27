---
title: "Run in Eval mode"
summary: "使用 calcit eval 求值代码片段、检查类型、加载外部模块。涵盖 --dep、--check-only、assert-type 等用法"
scope: "core"
kind: "reference"
category: "run"
aliases:
  - "eval"
  - "snippet"
  - "repl"
  - "type check"
  - "eval dep"
  - "eval module"
  - "calcit eval"
entry_for:
  - "calcit eval"
  - "calcit eval --dep"
  - "calcit eval --stdin"
  - "calcit eval --check-only"
---

# Run in Eval mode

短代码可以直接作为 `calcit eval '<snippet>'` 的位置参数。带引号的多行位置参数仍受支持（见下方示例）；需要避免 Shell 转义或从管道读取时，推荐使用 `calcit eval --stdin`。两种输入方式不能同时使用。旧顶层 `calcit exec` 已移除。

`eval` 会先检查整个片段，再执行其中的表达式。字面量 List 的 `&` 展开沿用普通参数合同：例如 `&list:concat ([] 1) & ([] ([] |wrong))` 会在执行前报告 Number/String 元素类型不匹配，和 Snapshot 中的同一调用一致。显式 `List<Dynamic>` 仍可用于保存和传递异构值，具体消费时再检查内容。

```bash
echo 'range 3' | calcit eval --stdin
```

Use `eval` command to evaluate code snippets from CLI:

```bash
$ calcit eval 'echo |demo'
1
took 0.07ms: nil
```

```bash
$ calcit eval 'echo "|spaced string demo"'
spaced string demo
took 0.074ms: nil
```

## Multi-line Code

You can run multiple expressions:

```bash
calcit eval '
-> (range 10)
  map $ fn (x)
    * x x
'
# Output: calcit version: 0.5.25
# took 0.199ms: ([] 0 1 4 9 16 25 36 49 64 81)
```

## 加载项目定义与外部模块

`eval` 为代码片段创建独立的运行上下文，包含 core 定义。使用 `--dep` 显式加载项目或模块；
目录路径以 `/` 结尾时读取其中的 `calcit.cirru`，也可以直接指定 Snapshot 文件。
仅包含旧 `compact.cirru` 的目录会被拒绝并提示迁移；先按 [升级指南](upgrade.md) 迁为 `calcit.cirru`，再加载。
加载定义不执行被加载项目的 `init-fn` 或 `reload-fn`；片段显式调用的函数仍按正常语义执行。

同一模块目录中的同名 Snapshot 经相对路径、绝对路径或目录软链接重复指定时，只加载一次并保留首个来源。不同目录或不同 Snapshot 文件名仍是独立来源；只软链接文件不会合并两个模块的资源目录，同名异内容 namespace 继续报冲突。

读取 `def answer 42` 这样的不可变值时，用逗号明确返回值，例如 `calcit eval --dep ./ ', app.main/answer'`；直接写一行 `app.main/answer` 是零参数调用，会因 Number 不是函数而报错。函数定义则按正常方式传参调用。

例如，已安装依赖的 [Corokia](https://github.com/calcit-lang/corokia) 项目有
`corokia.complex/c+` 定义。在该项目根目录执行：

```bash
calcit eval --dep ./ 'corokia.complex/c+ ([] 1 2) ([] 3 4)'
# 结果：([] 4 6)，不启动画布应用

# 等价的显式文件写法
calcit eval --dep ./calcit.cirru 'corokia.complex/c+ ([] 1 2) ([] 3 4)'
```

`calcit calcit.cirru eval '<snippet>'` 中的位置 Snapshot **不会自动成为 eval 的依赖**；
访问项目定义仍需 `--dep`。`range`、`let` 等 core 表达式无需加载项目，不能用它们验证项目上下文是否已加载。

外部模块同样使用 `--dep`，可重复传入。片段的首个表达式若为 `ns`，其中的 `:require`
规则会合并到片段自己的 namespace；它不会覆盖已加载项目的 `app.main` 或其他定义。例如在 Corokia 项目根目录导入同一项目定义：

```bash
calcit eval --dep ./ '
ns app.demo
  :require
    corokia.complex :refer $ c+

c+ ([] 1 2) ([] 3 4)
'
# 结果：([] 4 6)
```

`--dep` 负责加载定义，`:require` 负责导入名字，两者职责不同；只有 `:require` 不会自动加载项目。

## Type Checking in Eval

Type annotations and static checks work in eval mode:

```bash
# Type mismatch will cause error
calcit calcit.cirru eval 'let ((x 1)) (assert-type x :string) x'
# Error: Type mismatch...

# Correct type passes
calcit calcit.cirru eval 'let ((x 1)) (assert-type x :number) x'
# Output: 1
```

## Common Patterns

### Quick Calculations

```bash
calcit eval '+ 1 2 3 4'
# Output: 10

calcit eval 'apply * $ range 1 6'
# Output: 120  ; factorial of 5
```

### Testing Expressions

```bash
calcit eval '&list:nth ([] :a :b :c) 1'
# Output: :b

calcit eval '&map:get ({} (:x 1) (:y 2)) :x'
# Output: (Option :some 1)
```

### Exploring Functions

```bash
# Check function signature
calcit eval 'type-of range'
# Output: :fn

# Test with sample data
calcit eval '-> (range 5) (map inc) (filter (fn (x) (> x 2)))'
# Output: ([] 3 4 5)
```

## Important Notes

### Syntax Considerations

- **No extra brackets**: Cirru syntax doesn't need outer parentheses at top level
  - ✅ `calcit eval 'range 3'`
  - ❌ `calcit eval '(range 3)'` (adds extra nesting)

- **Let bindings**: Use paired list format `((name value))`
  - ✅ `let ((x 1)) x`
  - ❌ `let (x 1) x` (triggers "expects pairs in list for let" error)

### Error Diagnostics

- Type warnings cause eval to fail (intentional safety feature)
- Check `.calcit/error.cirru` for complete stack traces
- Use `calcit cirru parse-oneliner` to debug parse issues

### Query Examples

Use `calcit query examples` to see usage examples:

```bash
calcit calcit.cirru query examples calcit.core/let
calcit calcit.cirru query examples calcit.core/defn
```

For markdown snippet validation (`docs check-md`), see [CLI Options](./cli-options.md#markdown-code-checking).
