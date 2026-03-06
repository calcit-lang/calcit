# 2026-0307-0142 `:unit` 类型 + 内建函数 schema 批量补全

## 改动概要

### 1. Rust 类型系统: `CalcitTypeAnnotation::Nil` (`:unit`)

在 `src/calcit/type_annotation.rs` 中添加新的 `Nil` variant, 对应 Cirru 关键字 `:unit`。

**使用场景**: 副作用函数 (side-effectful functions), 明确标注返回 `nil` 的情况, 区别于 `:dynamic` (未知类型)。

**涉及改动点**:
- `CalcitTypeAnnotation` enum: 新增 `Nil` variant
- `builtin_type_from_tag_name`: `"unit" | "nil" => Self::Nil`
- `builtin_tag_name`: `Self::Nil => Some("unit")`
- `variant_order`: `Self::Nil => 27`
- `matches_with_bindings`: `(Self::Nil, Self::Nil) => true`
- `Hash impl`: `Self::Nil => "nil".hash(state)`
- `value_matches_type_annotation`: `CalcitTypeAnnotation::Nil => matches!(value, Calcit::Nil)`
- `gen_ir.rs`: `CalcitTypeAnnotation::Nil => type_tag_map("unit")`

### 2. 副作用函数 `:return :unit` schema

在 `src/cirru/calcit-core.cirru` 中为以下函数补充 `:return :unit` schema:

| 函数名 | schema 要点 |
|--------|------------|
| `each` | `:args ([] :dynamic :fn) :return :unit` |
| `write-file` | `:args ([] :string :string) :return :unit` |
| `quit!` | `:args ([] (:: :optional :number)) :return :unit` |
| `add-watch` | `:args ([] :ref :dynamic :fn) :return :unit` |
| `remove-watch` | `:args ([] :ref :dynamic) :return :unit` |
| `reset!` | `:generics 'T :args ([] (:: :ref 'T) 'T) :return :unit` |

**注意事项**: 在 `:: :fn ('T) :unit` 中, `'T` 出现在 `('T)` 列表的算子位置会触发 EDN 解析错误 (`invalid operator for edn: 'T`)。对 `each` 的解决方案是简化成 `:fn`, 避免在 fn 参数类型中使用泛型变量作为算子。

### 3. 内建函数 schema 批量补全 (54 条)

对 `calcit-core.cirru` 中原本 `:schema nil` 的内建 primitive 函数批量补充类型:

**`&list:*` (16 条)**: `assoc`, `assoc-after`, `assoc-before`, `concat`, `contains?`, `count`, `dissoc`, `distinct`, `empty?`, `first`, `includes?`, `nth`, `rest`, `reverse`, `slice`, `to-set`

**`&map:*` (12 条)**: `assoc`, `common-keys`, `contains?`, `count`, `destruct`, `diff-keys`, `diff-new`, `dissoc`, `empty?`, `get`, `includes?`, `to-list`

**`&str:*` (15 条)**: `compare`, `concat`, `contains?`, `count`, `empty?`, `escape`, `find-index`, `first`, `includes?`, `nth`, `pad-left`, `pad-right`, `replace`, `rest`, `slice`

**集合操作 (10 条)**: `&difference`, `&exclude`, `&include`, `&set:count`, `&set:destruct`, `&set:empty?`, `&set:includes?`, `&set:intersection`, `&set:to-list`, `&union`

**泛型模式** (使用 `'T` 非算子位置):
```cirru
{} (:kind :fn)
  :generics $ [] 'T
  :args $ [] (:: :list 'T) :number 'T
  :return $ :: :list 'T
```

### 4. Clippy & 代码质量修复

- `emit_js.rs`: 消除冗余闭包 `xs.iter().skip(1).any(schema_marks_async)`
- `query.rs`: 添加复杂类型别名 `type RefResults = ...`

## 技术知识点

### `'T` 在 schema 文件中的限制

- **可用位置**: `:generics $ [] 'T`, `(:: :list 'T)` 的末尾参数, `[] :arg1 'T`
- **不可用位置**: `('T)` 即 `'T` 作为列表算子 (head), 例如 `(:: :fn ('T) :unit)` 中的 `('T)`
- **根本原因**: schema 验证通过 `cirru_edn::parse` 进行, EDN 解析器要求列表头部必须是合法算子

### `:unit` vs `:nil` vs `:dynamic`

- `:unit` — 明确标注 "此函数返回 nil, 且是预期行为" (副作用函数)
- `:nil` — 内部等价于 `:unit` (通过 `"unit" | "nil"` 分支解析)
- `:dynamic` — 未知/任意类型 (默认回退)

## 验证结果

```
cargo clippy -- -D warnings  → ✓ 0 warnings
cargo test                   → ✓ 17/17 passed
yarn check-all               → ✓ took 571ms, all passed
```
