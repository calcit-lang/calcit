# 2026-03-05 calict.core 泛型签名细化

## 目标

在不改变运行时语义的前提下，继续收紧 `calcit-core.cirru` 中 `defn` 的类型签名，让类型推断更稳定、错误提示更精确。

## 主要改动

本次聚焦于“保型列表函数”“可选返回值函数”“函数组合器”“map 回调函数形状”四类。

### 一、列表保型与可选返回

- `distinct`, `drop`, `repeat`, `reverse`, `take`, `take-last`
  - 统一到 `hint-fn (generics 'T) $ return-type (:: :list 'T)`。
- `&list:find-last`
  - 改为 `(:: :optional 'T)`。
- `&list:find-last-index`, `&list:last-index-of`, `index-of`
  - 改为 `(:: :optional :number)`。
- `&list:max`, `&list:min`
  - 改为 `(:: :optional 'T)`。

### 二、列表/映射映射函数泛型化

- `&list:map`
  - 从宽泛 `:list/:fn` 收紧到 `('T -> 'U)`，返回 `(:: :list 'U)`。
- `map`
  - 回调从 `('T -> 'T)` 扩展为 `('T -> 'U)`，与 map 语义一致。
- `map-indexed`
  - 返回类型显式为 `(:: :list 'U)`，回调签名改为 `(:number 'T) -> 'U`。

### 三、拼接与组合函数

- `concat`, `conj`, `interleave`, `join`
  - 输入与输出统一到同一元素类型 `'T` 的列表。
- `&list:apply`
  - 收紧为 `xs: list<T>, fs: list<(T -> U)>`，返回 `list<U>`。

### 四、函数组合器与 map helper

- `&fn:apply`, `&fn:bind`, `&fn:map`
  - 增加 `('A 'B 'C)` 级别泛型，表达组合器的真实输入输出关系。
- `&map:map-list`
  - 返回改为 `(:: :list 'U)`，并补充 `f/acc/pair` 类型约束。
- `&list:map-pair`
  - 返回改为 `(:: :list 'U)`，`f` 改为 `('K 'V) -> 'U`。
- `&map:filter`, `&map:filter-kv`, `&map:map`
  - 回调签名改为带输入/输出形状的泛型函数类型，减少 `:fn` 过宽带来的误判。

## 验证

日志均写入仓库内 `js-out/`：

- `yarn check-all > js-out/check-all.log`，`EXIT:0`
- `cargo build --release > js-out/release-build.log`
- `./target/release/cr calcit/test.cirru > js-out/release-run.log`，`EXIT:0`

结论：本次改动仅增强静态类型信息，未改变运行行为，且全量检查通过。
