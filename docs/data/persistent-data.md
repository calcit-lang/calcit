---
title: "Persistent Data"
scope: "core"
kind: "reference"
category: "data"
aliases:
  - "persistent collections"
  - "immutable collections"
  - "finger vec"
id: core/data/persistent-data
parent: core/data
---
# Persistent Data

Calcit 的 List、Map 与 Set 都是持久化不可变结构：每次修改返回新值，旧值保持不变，并与新值共享未改动的部分。

| 集合 | Rust runtime | JavaScript runtime |
| ---- | ------------ | ------------------ |
| List | [finger-vec.rs](https://github.com/calcit-lang/finger-vec.rs)（`FingerVec`） | [`@calcit/finger-vec`](https://www.npmjs.com/package/@calcit/finger-vec)，追加密集场景另有 `CalcitSliceList` 快速路径 |
| Map | [rpds](https://github.com/orium/rpds) `HashTrieMap` | [ternary-tree.ts](https://github.com/calcit-lang/ternary-tree.ts) 的 `TernaryTreeMap` |
| Set | rpds `HashTrieSet` | 基于 `TernaryTreeMap` 的实现 |

`&format-ternary-tree` 已在 0.33.0 删除：它沿用了旧的三叉树名字，实际输出 FingerVec 的调试表示，见 [升级手册](../run/upgrade.md#format-ternary-tree-removal)。
