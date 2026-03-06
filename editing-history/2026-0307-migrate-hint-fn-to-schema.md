# 2026-03-07 migrate hint-fn to schema for top-level defs

## 背景

`calcit-core.cirru` 中大量顶层函数在函数体内以 `hint-fn $ {} (:return ...)` 的方式标注返回类型。这是旧写法，新写法是在 `:schema` 字段中声明 `{} (:kind :fn) (:return ...)` 等结构。

## 本次操作

- 将所有顶层定义（`defn`/`defmacro`）的 `hint-fn` 从函数体 `[3,0]` 位置删除，改为 `edit schema` 写入 `:schema` 字段
- 共处理 100+ 个函数，包含 `calcit.core` 和 `calcit.internal` 两个 namespace
- 对带泛型的函数（`&list:filter`、`&list:map`、`&fn:bind` 等）补充了 `:generics` 字段

## 保留 hint-fn 的情况

内层/局部函数没有 schema 位置，仍然保留 hint-fn，例如：

- `{,}` 内的 `&{,}` 局部函数
- `map` 内的 `%map`
- `join` 内的 `%join` / `%join-str`
- `select-keys` 内的 `%select-keys`
- `&map:filter` 等内部 defn

## 修复的 bug

- `parse_target` 在 `query.rs` / `edit.rs` 中使用 `rsplit_once('/')` 导致函数名含 `/`（如 `/`、`/=`）无法识别，改为 `split_once('/')` 修复

## 验证

`yarn try-rs` 全程通过，最终耗时约 350ms。
