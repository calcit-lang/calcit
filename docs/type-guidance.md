---
title: "Type Guidance"
summary: "Dynamic 审计、Option/Result 组合、嵌套数据访问和类型化 Enum 构造"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "dynamic audit"
  - "option result"
  - "typed enum"
id: core/features/type-guidance
parent: core/features
---

# Calcit 类型使用指南

## Dynamic 是边界，不是默认多态

`Dynamic` 适合 JS FFI、框架开放数据、宏和确实无法提前知道的外部输入。普通函数不要用多个 `Dynamic` 表示“它们应该是同一个类型”：输入和返回关联时用 `:generics` 与 TypeVar；只需要能力时用 trait 与 `:where`；同质集合写出元素类型；有限异构数据定义为 Enum；可缺失值使用 `Option<T>`，带失败信息使用 `Result<T, E>`。

每次执行和编译会在 stderr 输出 Dynamic 用量提示。它是趋势信号，不会替代具体路径检查：

```bash
cr analyze check-types --summary-only
cr analyze weak-types --only schema-dynamic,code-dynamic --intent unresolved --format json
```

## Option / Result 组合

优先组合而不是逐层 `unwrap`：

```cirru.no-check
option:and-then user
  fn (user)
    option:and-then (get user :profile)
      fn (profile) $ get profile :name

result:and-then loaded
  fn (value) $ validate value
```

备用来源使用 `option:or-else` / `result:or-else`。`unwrap-or` 只用于默认值，`map` 用于同步转换，`and-then` 用于下一个仍可能失败的操作。

## get-in / update-in

`get-in` 是可能失败的开放数据访问，返回 `Option<T>`。不要用它绕过 Struct 字段检查；Struct 路径应使用 `(:field value)`，字段可缺失就把字段声明为 `Option<T>`。

`update-in` 的 updater 接收 `Option<T>`。对缺失值给默认值或明确返回 `%none`，不要无条件 unwrap：

```cirru.no-check
update-in data ([] :settings :retries)
  fn (current)
    option:unwrap-or current 0
```

## Enum 构造

已知 Enum 定义时使用头部调用：

```cirru.no-check
Option :some value
Result :err message
```

Calcit 会根据 Enum 定义检查 variant 和 payload，并在预处理阶段生成命名构造。`%::` 保留给显式 prototype、动态跨模块构造和兼容旧代码的边界。
