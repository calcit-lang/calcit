# Unify checked collection call contracts / 统一已检查集合调用契约

Issue: calcit-lang/calcit#797

## English

- Added one receiver-specialized checked call contract for the first `get`,
  `update`, `filter`, and `map` batch. The same bound receiver, argument,
  callback, return, and lowering evidence now feeds callback preprocessing,
  argument checking/rewriting, return inference, and static lowering.
- Removed the replaced collection-name tables from checking, inference, and
  lowering. Direct Struct `update` remains on its existing nominal fallback;
  Map pair callbacks remain explicitly open as heterogeneous Lists.
- Kept Dynamic and unresolved type-slot receivers on the compatibility path.
  Map lookup preserves nested `Option` payloads instead of flattening them,
  and cross-namespace nominal key identity is retained.
- Extended the public type-fail characterization with named, inline, and
  generic callback contrasts. Inline callbacks now receive the concrete member
  type before their inferred return is checked; expected output never proves an
  unknown generic payload.

## 中文

- 为首批 `get`、`update`、`filter`、`map` 增加统一的 receiver-specialized
  checked call contract；同一份 receiver、参数、callback、返回值与 lowering
  绑定证据现在同时供 callback 预处理、参数检查/重写、返回推断和静态降级使用。
- 删除 checking、inference、lowering 中已被替代的集合名称匹配表。Struct
  `update` 继续走既有 nominal fallback；Map pair callback 继续保持明确开放的
  heterogeneous List 契约。
- Dynamic 与未解析 type slot receiver 继续走兼容路径。Map lookup 不会把字段
  自身的嵌套 `Option` flatten，并保留跨 namespace nominal key 身份。
- 扩充公共 type-fail characterization，覆盖具名、inline、generic callback
  对照。inline callback 会先获得具体成员类型再检查推断返回值，expected output
  不会反向证明未知 generic payload。

## Validation / 验证

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test` (735 + 308 + 23 passed)
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`: all 18 scenarios passed; warm per-scenario
  timings stayed in the same range as main (the largest scenario was faster,
  1123 ms versus 1455 ms), with identical stdout sizes.
