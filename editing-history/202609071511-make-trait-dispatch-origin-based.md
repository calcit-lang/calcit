# Make strict trait dispatch origin-based / 严格 trait 分发改用 nominal origin

Issue: calcit-lang/calcit#844

## 中文

- 抽出统一 method candidate resolution，供参数/签名检查、返回类型推断、静态内联和冲突诊断共同使用；候选按 `definition_ref` 或 runtime identity 区分，不再用 trait 短名合并。
- strict mode 对不同 trait origin 的同名方法报告 `E_AMBIGUOUS_TRAIT_METHOD`，对同一 origin 的重复 impl 报告 `E_DUPLICATE_TRAIT_IMPL`，对 originless legacy method bag 报告 `E_ORIGINLESS_METHOD_DISPATCH`。compat mode 保留 builtin first-wins / user last-wins，并用完整 origin 报警。
- `&trait-call` 继续按精确 trait origin 选择；strict runtime 同样拒绝同一 trait 的重复 impl。
- requires closure 按 nominal origin 去重并稳定排序，检测并报告 origin 路径循环；可达 method/default schema 不完整或不满足声明时不再作为 `:where` 的 Proven 证据。
- compiler-generated nominal callable 新增 trait definition dependency，使 impl、default 或 method schema 变化能在增量编译/hot reload 时失效旧静态分发结果。
- 更新 trait 文档，明确 strict/compat 规则、错误码、迁移方式和 origin-qualified 诊断。
- 用当前工作树 binary 对本地 Respo 与 Recollect 执行 `--compat-types --warn-dyn-method --check-only`：两者实际 multiple-origin conflict inventory 均为 0，因此未虚构或迁移消费者调用；Respo 仅有 3 个既有 unresolved `.to-list` finding，Recollect（含依赖）同样只有这 3 个 unresolved finding。

## English

- Added one shared method-candidate resolution path for argument/signature checking, return inference, static lowering, and conflict diagnostics. Candidates use `definition_ref` or runtime identity instead of the printed trait short name.
- Strict mode reports `E_AMBIGUOUS_TRAIT_METHOD` for competing trait origins, `E_DUPLICATE_TRAIT_IMPL` for duplicate implementations of one origin, and `E_ORIGINLESS_METHOD_DISPATCH` for legacy originless method bags. Compatibility mode retains built-in first-wins and user last-wins behavior while warning with fully qualified origins.
- Kept `&trait-call` as the exact-origin selector and made strict runtime calls reject duplicate implementations of the selected trait.
- Normalized and de-duplicated requires closures by nominal origin, reported cycle paths, and prevented incomplete reachable method/default schemas from becoming Proven `:where` evidence.
- Added the trait definition as a dependency of compiler-generated nominal callables so impl/default/schema edits invalidate stale static dispatch across incremental compilation and hot reload.
- Documented the strict/compat rules, stable diagnostics, and migration path.
- Audited local Respo and Recollect with the current binary using `--compat-types --warn-dyn-method --check-only`. Both real multiple-origin inventories are zero, so no consumer conflict was invented or migrated. The only method findings were the same three unresolved `.to-list` sites from Respo (also visible through Recollect dependencies).

## Verification / 验证

- `cargo test --workspace --all-features` (747 library, 308 native CLI, 23 WASM, and 0 doc tests passed)
- focused exact-origin, duplicate-impl, originless, requires-cycle, invalid/default-schema, compatibility-order, cache-dependency, and explicit `&trait-call` tests
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `yarn compile`
- `yarn check-all` (native, JS, IR, WASM, and benchmark smoke checks passed)
- `yarn check-agent-interface` (18/18 passed as part of the full gate)
- Respo/Recollect compatibility inventory commands above; consumer worktrees unchanged
