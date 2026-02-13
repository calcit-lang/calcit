# 202602131841 trait origin 匹配与结构化 trait 相等性

## 背景

本次提交延续了动态方法告警收敛与 trait 分发可靠性改进。
评审中提出的核心问题是：trait 分发不应只依赖 trait 名称匹配。
`impl` 记录本身已经携带了 trait 的 origin 元数据，因此匹配应直接基于 origin。

## 目标

1. 让 `&trait-call` 按 trait origin 相等性选择 impl，而不是按名称字符串匹配。
2. 让 trait/impl 的相等语义与上述运行时选择模型一致。
3. 在尽量保持兼容的前提下，提高歧义/重名场景下的正确性。
4. 保持现有优先级规则不变（用户 impl：后者覆盖前者；内建 impl：前者优先）。

## 思考过程

### 为什么 name-only 匹配不够

- 仅按名称匹配会把“同名但结构不同”的 trait 混为一类。
- 运行时已有 `CalcitImpl.origin: Option<Arc<CalcitTrait>>`，若忽略它会浪费已有语义信息。
- 使用显式 `&trait-call` 时，调用方预期的是“精确 trait 命中”，而非按名字近似匹配。

### 分发策略决定

在 `trait_call` 中：

- 旧逻辑：`imp.trait_name()` 与 `trait_def.name` 比较。
- 新逻辑：`imp.origin()` 与目标 trait 直接通过 `Eq` 比较。

该方案改动小、语义清晰，并与当前运行时模型一致，不引入额外解析复杂度。

### Eq/Hash 一致性

当分发切换到 origin 相等后，`CalcitTrait` 的相等语义就不能继续只看 `name`。

因此将 `CalcitTrait` 的 Eq/Hash 升级为结构化比较，包含：

- trait 名称
- 方法名列表
- 方法类型签名
- requires（依赖 trait）
- 默认实现槽位形状（每个位置是 `Some` 还是 `None`）

关于默认实现：

- 默认函数体（`CalcitFn`）不做深比较/深哈希，因为 `CalcitFn` 当前不提供完整 Eq/Hash。
- 这里只比较默认槽位可用性形状，以保证 Eq/Hash 契约一致，同时提升结构判断准确性。

## 文件级改动

### 运行时分发与文档

- `src/builtins/meta.rs`
  - `trait_call` 改为按 `impl.origin` 相等匹配。
  - 同步更新函数附近注释，确保文档与实现一致。

### impl 相等性

- `src/calcit/calcit_impl.rs`
  - `PartialEq` 改为直接比较 `origin`（不再只比较 `origin.name`）。

### trait 相等性与哈希语义

- `src/calcit/calcit_trait.rs`
  - `PartialEq` 从 name-only 改为结构化比较。
  - `Hash` 同步到相同结构维度。
  - 增加注释，说明默认函数比较策略。

### 相关一致性修正（同一工作区变更）

- `src/runner/preprocess.rs`
  - 修复重命名后的测试守卫符号残留（`DynTraitCheckGuard` -> `WarnDynMethodGuard`）。

## 验证

- `cargo fmt`
- `cargo test -q`
  - 结果：当前工作区下测试全部通过。

## 兼容性与风险

- 该行为变化是有意为之：同名但结构不同的 trait 不再被视为相等。
- 默认函数体差异仍不纳入 Eq/Hash。
- 方法分发优先级规则保持不变。

## 后续可选项

1. 如有需要，可补强默认实现身份比较（例如优先比较 def ref）。
2. 增加“同名不同结构 trait”相等性的显式测试用例。
3. 继续收敛 `warn-dyn-method` 全量运行中的剩余动态调用告警。
