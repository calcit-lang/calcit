# RFC: Register API / Host Capability 规范化（与当前实现对齐）

状态：Draft  
作者：Copilot（基于当前代码审阅）  
日期：2026-02-17

---

## 1. 背景

Calcit 当前已经具备两套能力：

- 内建 proc/syntax（语言核心）
- 宿主注入 proc（`register_import_proc*`）

同时 FFI 入口已采用旧命名风格并在当前代码中稳定运行：

- `&call-dylib-edn`
- `&call-dylib-edn-fn`
- `&blocking-dylib-edn-fn`

ABI 版本当前为：`0.0.9`（保持不变）。

---

## 2. 当前事实（对应现状）

### 2.1 注册链路

- 注册表：`IMPORTED_PROCS`
- 元数据表：`IMPORTED_PROC_DESCRIPTORS`
- 注册入口：
  - `register_import_proc(name, f)`
  - `register_import_proc_with_descriptor(name, f, descriptor)`
- 调用入口：`call_registered_proc(alias, args, call_stack)`

### 2.2 已落地校验能力

descriptor 已用于统一校验：

- 参数个数（`arity_min` / `arity_max`）
- 平台可用性（`platforms`）
- 回调最后位约束（`callback_last`）
- 稳定级别 warning（`stability` + once-warning）
- 副作用 / 分析标签（`tags`，与 `calcit.core` CodeEntry `:tags` 同名，如 `:log`、`:io`、`:interop`）

### 2.3 FFI 注入与命名

当前注入保持旧命名风格：

- `&call-dylib-edn`
- `&call-dylib-edn-fn`
- `&blocking-dylib-edn-fn`

这与 `dylib-workflow` 当前调用方式一致。

---

## 3. 目标（不引入破坏性变更）

1. 保持旧 FFI 命名不变，减少用户侧迁移成本。
2. 在不改 ABI（`0.0.9`）前提下继续完善 descriptor 治理能力。
3. 统一错误语义，避免 FFI 边界 panic 直接外露。
4. 为后续平台能力文档化提供稳定基础。

---

## 4. 非目标

- 不在当前阶段切换为新命名空间形式（如 `calcit.ffi/*`）。
- 不在当前阶段升级 ABI 主/次版本。
- 不尝试统一 Rust 与 JS 的异步模型。

---

## 5. 建议路线

### Phase A（当前可持续）

- 继续使用旧 FFI 命名。
- 把 descriptor 作为 host proc 的默认治理入口。
- 保持 ABI `0.0.9`。

### Phase B（增强稳定性）

- 统一 FFI 错误映射（符号缺失、ABI 不匹配、参数错误）。
- 补齐文档中的平台可用性与调用约束说明。

### Phase C（未来可选）

- 若确有收益，再讨论命名空间 API 与 ABI 演进。
- 必须先提供迁移文档与自动检查脚本，再做切换。

---

## 6. 验收标准（当前版本）

1. `dylib-workflow` 在旧命名下可直接 `cr -1` 通过。
2. `call_registered_proc` 校验（arity/platform/callback）持续生效。
3. ABI 版本保持 `0.0.9` 且调用链稳定。
4. 相关测试保持通过（Rust tests + workflow 冒烟）。

---

## 7. 决策记录（本轮）

- 采用旧命名风格优先（兼容优先）。
- 保留并继续使用 descriptor 机制（治理优先）。
- ABI 不变（风险控制优先）。

---

## 8. 后续维护建议

- 任何涉及 FFI 命名或 ABI 的讨论，都应先更新本 RFC。
- 若未来开启迁移，需在本 RFC 中追加“兼容窗口 / 回滚策略 / CI 检查项”。
