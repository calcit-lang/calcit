# Prevent recursive generic bindings

## Context / 背景

While rebuilding `Cumulo/cumulo-reel.calcit` with Calcit 0.13.75, JS codegen
overflowed its 64 MiB worker stack while preprocessing
`recollect.wasm-test/probe-map-keys`. The minimal trigger combines nested generic
collection access with homogeneous equality:

```cirru
= (&list:first (&list:first (&map:to-list ({} (:score 1))))) :score
```

在 Calcit 0.13.75 下重新编译 `Cumulo/cumulo-reel.calcit` 时，JS codegen 在补全
`recollect.wasm-test/probe-map-keys` 的预处理结果时耗尽 64 MiB worker stack。

## Root cause / 根因

Generic matching could bind `T` to a type that recursively contains `T`, such
as `Optional<T>`. When a later argument reused `T`, matching repeatedly followed
the cyclic binding and never terminated.

泛型匹配此前可能记录 `T = Optional<T>` 一类包含自身的绑定；后续参数再次使用
`T` 时会无限递归解析。

## Change / 修改

- Add a named type-variable occurs-check across container, function, nominal,
  optional/nullish and variadic annotations.
- Follow aliases already present in the binding graph so indirect cycles such
  as `T = U` followed by `U = Optional<T>` are rejected as well.
- If a candidate contains the variable being bound, keep the unresolved
  generic match permissive without storing the recursive binding. Later
  concrete arguments can still bind the variable normally.
- Cover the nested binding explicitly and confirm the original minimal case now
  reports its ordinary type warning instead of aborting.

Tracking: calcit-lang/calcit#596.
