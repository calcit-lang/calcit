# Strict implicit nil boundaries / strict 隐式 nil 边界

## Summary / 概要

- Added a dedicated strict-preprocess flag wired from `--strict-types`.
- Added stable hard diagnostics for legacy `?` parameters
  (`E_LEGACY_OPTIONAL_PARAM`) and partial Struct constructors
  (`E_PARTIAL_STRUCT_NIL_FILL`).
- Kept both legacy behaviors available in ordinary mode so ecosystem migration
  can proceed incrementally.
- Preserved user-source locations through `fn` macro expansion by reusing the
  preferred macro call-site location.
- Added compatibility/strict branch tests, migration documentation, and a
  first text-level census of Calcit workspace and ecosystem call sites.

## Design notes / 设计要点

Trailing parameters declared as `Option<T>` already support deterministic
omission by inserting `%none`; therefore the compiler can give a complete
migration for `?` without retaining nil binding in strict code. `%{}?` cannot be
auto-fixed safely because an omitted field may need `%none`, a domain default,
or a caller-provided value, so the diagnostic only gives explicit alternatives.

尾部 `Option<T>` 参数已经支持缺参自动补 `%none`，因此 strict 模式可以完整替代旧
`?` 的 nil 注入。Struct 缺失字段的业务语义无法由编译器猜测，所以 `%{}?` 只提供
定位和迁移建议，不做自动重写。

## Verification / 验证

- Targeted Rust tests cover ordinary compatibility and strict rejection for
  both constructs.
- Manual `calcit eval --strict-types` checks cover macro-expanded `fn (? x)` and
  both `%{}?` / `&%{}?`, including user call-site locations.
