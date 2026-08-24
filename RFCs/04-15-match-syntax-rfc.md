# RFC: Native `match` Syntax with Exhaustiveness Checking

状态：Implemented
日期：2026-04-15

---

## 1. 概要

引入新的原生语法 `match`，用于对 enum（tagged union）进行模式匹配。与现有 `tag-match` 宏不同，`match` 在预处理阶段执行穷尽性检查（exhaustiveness checking），在运行时直接执行分支匹配，在 JS 代码生成中输出高效的 if-else 链。

## 2. 动机

### 问题

`tag-match` 是定义在 `calcit-core.cirru` 中的宏，展开后变成嵌套的 `if` + `identical?` 表达式。宏展开后：

- **丢失结构信息**：预处理器只看到 `if`/`identical?`，无法知道"这是一个多分支枚举匹配"
- **无法做穷尽性检查**：展开后的代码无法回溯到原始的分支列表
- **无法校验变体存在性**：拼错 tag 名只会得到运行时错误
- **无法校验 arity**：绑定数量与变体定义不匹配只会运行时报错

### 解决方案

将 `match` 作为原生语法（`CalcitSyntax::Match`），在编译器各阶段都能看到完整的分支结构：

```
源码 → 预处理（穷尽性+类型推断） → 运行时（直接分支匹配） → JS（if-else 链）
```

## 3. 语法设计

### 基本形式

```cirru
match <value>
  (<pattern1>) <body1>
  (<pattern2> <binding1> <binding2>) <body2>
  _ <default-body>
```

在 Cirru 的缩进规则下，每一行的 `pattern` 和 `body` 构成一个二元组 `(pattern body)`，自然映射为 AST 中的 pair。

### Pattern 形式

| Pattern            | 含义                       | 示例                       |
| ------------------ | -------------------------- | -------------------------- |
| `(:tag)`           | 零载荷变体                 | `(:ok) :success`           |
| `(:tag b1 b2 ...)` | 带载荷变体，绑定到局部变量 | `(:err msg) (println msg)` |
| `_`                | 通配符（匹配所有）         | `_ :default`               |

### 完整示例

```cirru
defenum Result0 (:ok) (:err :string)

let
    r $ %:: Result0 :err |oops
  match r
    (:ok) :success
    (:err msg) msg
; => |oops
```

### 与 `tag-match` 的对比

`match` 和 `tag-match` 的分支书写格式完全相同（都是 `(pattern body)` 对），迁移仅需将 `tag-match` 关键字替换为 `match`：

```cirru
; Before
tag-match r
  (:ok) :success
  (:err msg) msg

; After
match r
  (:ok) :success
  (:err msg) msg
```

## 4. 预处理阶段

### 类型推断

通过 `infer_type_from_expr` 从值表达式推断枚举类型：

1. 直接类型标注 → `CalcitTypeAnnotation::Tuple(enum_ref)` → 得到 `CalcitEnum`
2. TypeSlot 引用 → 解析后得到 `CalcitTypeAnnotation::Enum(e, _)` → 得到 `CalcitEnum`
3. 推断失败 → 跳过穷尽性检查（仍可在运行时匹配）

### 变体校验

对每个 `(:tag binding ...)` pattern：

- 检查 `tag` 是否存在于枚举定义的 variants 中
- 检查 binding 数量是否匹配 variant 的 `arity()`
- 不匹配时产生 `[Warn]` 预处理警告

### 穷尽性检查

收集所有分支覆盖的 tag → 与枚举的全部 variant tag 做集合差：

```
missing = all_variants - covered_tags
```

若 `missing` 非空且无 `_` 通配符，产生警告：

```
[Warn] match on `Result0` is not exhaustive. Missing variant(s): [:ok]
```

### 绑定类型推断

对 pattern 中的每个 binding symbol，从 `EnumVariant::payload_types()` 获取对应位置的类型标注，创建 `CalcitLocal` 并注入 body 的作用域类型表。

## 5. 运行时行为

`syntax_match` 的执行流程：

1. 求值 `value` 表达式
2. 解构为 `CalcitTuple { tag, extra, .. }`
3. 遍历分支对 `(pattern body)`：
   - 若 pattern 是 `_` → 匹配成功，求值 body 返回
   - 若 pattern 是 `(:tag b1 b2 ...)` 且 tag 相等且 arity 匹配 → 创建作用域绑定 `b1=extra[0], b2=extra[1], ...`，求值 body 返回
4. 所有分支都不匹配 → 运行时错误 `"match: no matching branch for tag :xxx"`

## 6. JS 代码生成

动态或无法安全重排的 match 保留立即执行函数中的 if-else 链。静态已知 enum 且分支没有重复 tag、wildcard 位于末尾时，预处理器会生成内部 declaration-order branch table；JS codegen 对该形式生成 `switch (tag.idx)`，native evaluator 通过 enum definition 的 variant index 直接选择 slot。这个内部表示不增加用户语法，匿名 enum 和动态边界仍使用兼容路径。

概念上的 JS 输出如下：

```javascript
(() => {
  let match_v = <value>;
  let match_t = _$n_tuple_$o_nth(match_v, 0);
  switch (match_t.idx) {
    case _kn_ok.idx:
      if (_$n_tuple_$o_count(match_v) === 1) return kwd_$o_success;
      break;
    case _kn_err.idx:
      if (_$n_tuple_$o_count(match_v) === 2) {
        let msg = _$n_tuple_$o_nth(match_v, 1);
        return msg;
      }
      break;
  }
  throw new Error("match: no matching branch for tag");
})()
```

通配符 `_` 分支生成无条件 `else` 块。

## 7. 实现文件

| 文件                           | 变更                                               |
| ------------------------------ | -------------------------------------------------- |
| `src/calcit/syntax_name.rs`    | `CalcitSyntax::Match` 变体 + `SyntaxTypeSignature` |
| `src/builtins/syntax.rs`       | `syntax_match()` 运行时处理器                      |
| `src/builtins.rs`              | dispatch 入口                                      |
| `src/runner/preprocess/mod.rs` | `preprocess_match()` 预处理+穷尽性检查             |
| `src/codegen/emit_js.rs`       | `gen_match_code()` JS 代码生成                     |
| `calcit/test-enum.cirru`       | 测试用例                                           |

## 8. 迁移指南

`tag-match` 和 `match` 的分支语法完全一致，迁移只需：

1. 将 `tag-match` 替换为 `match`
2. 确保被匹配的值有明确的枚举类型标注（以启用穷尽性检查）
3. 根据编译器的穷尽性警告补充遗漏的分支或添加 `_` 通配符

`tag-match` 仍然可用，不会被移除。

## 9. 限制与未来工作

- **当前**：类型推断依赖函数 schema 或显式 `%::` 构造，局部变量的枚举类型有时无法推断
- **当前**：不支持嵌套 pattern（如 `(:ok (:inner x))`）
- **未来**：可扩展为支持 struct/record 的解构匹配
- **未来**：可添加 `match` 表达式的返回类型推断（从所有分支 body 计算 union type）
