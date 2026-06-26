# calcit-core / RegisteredProcDescriptor effect tags

## 概要

为 `cr analyze effects-graph` 静态分析补充 effect 标记，数据源分两处、tag 名称统一：

1. **`calcit-core.cirru`** — builtin CodeEntry 的 `:tags`
2. **宿主注入 proc** — `RegisteredProcDescriptor.tags`（`HashSet<EdnTag>`）

RFC `06-15-effects-graph-rfc.md` §4.2 已同步完整 tag 约定表。

## calcit.core Tag 分类

| Tag | 目标 |
|-----|------|
| `:state` | `defatom`, `atom`, `reset!`, `swap!`, `deref`, `ref?`, `add-watch`, `remove-watch`, `&atom:deref`, `&buf-list:*`, `&buffer`, `deftype-slot`, `with-type-slot` |
| `:io` | `read-file`, `write-file`, `get-env`, `cpu-time`, `&get-os`, `&get-calcit-*`, `generate-id!` |
| `:file` | `read-file`, `write-file`（与 `:io` 叠加） |
| `:env` | `get-env`（与 `:io` 叠加） |
| `:control` | `raise`, `quit!`, `try`；`assert`/`assert=`/`assert-detect`（失败时 `raise` + `eprintln`） |
| `:log` | `&display-stack`, `with-cpu-time`（宿主 `println`/`eprintln`/`echo` 见下节） |
| `:meta` | `&get-def-doc`, `&get-def-schema`, `macroexpand*`, `assert-type`, `deftype-slot`, `with-type-slot`, `&data-to-code`, `&extract-code-into-edn` |
| `:async` | `hint-fn` |
| `:watch` | `add-watch`, `remove-watch`（与 `:state` 叠加） |
| `:effect` | `&doseq` |
| `:interop` | `eval`, `js-object` |

## RegisteredProcDescriptor

`println` / `eprintln` / `echo` **不能**在 core 中用 `&runtime-implementation` 建 stub（会与 `has_def_code` 冲突）；改由 descriptor 标记。

### API

- `builtins::proc_tags(["log", "io"])` — 构建 tag 集合
- `registered_proc_descriptor(name)` / `list_registered_procs()` / `registered_proc_has_tag(name, tag)`
- `cr query host-procs [--tag :log]`

### 已标记注入 proc

| Proc | Tags |
|------|------|
| `println` / `eprintln` / `echo` | `:log` `:io` |
| `&call-dylib-edn*` | `:interop` `:io` |
| `async-sleep` | `:io` |
| `on-control-c` | `:control` `:io` |

### 其它实现细节

- `inject_platform_apis()` 幂等；在 `main` 开头调用，保证 `cr query` 可见已注入 proc。

## 查询示例

```bash
cr src/cirru/calcit-core.cirru query defs calcit.core --tag :state
cr src/cirru/calcit-core.cirru query defs calcit.core --tag :meta
cr calcit/test.cirru query host-procs --tag :log
```
