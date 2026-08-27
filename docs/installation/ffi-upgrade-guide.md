---
title: "FFI 项目升级手册"
scope: "core"
kind: "guide"
category: "installation"
aliases:
  - "ffi upgrade"
  - "dylib upgrade"
---

# FFI 项目升级手册

本文描述将 Calcit FFI 动态库项目（dylib 工程）升级到最新依赖版本的完整流程，基于实际升级经验整理。

## 版本与协议

每个 FFI 项目需要同步维护两处版本号：

| 文件         | 字段                      | 说明                                                        |
| ------------ | ------------------------- | ----------------------------------------------------------- |
| `Cargo.toml` | `cirru_edn = "x.y.z"`     | 模块内部使用的 Cirru EDN codec；不再与 Calcit host 的 Rust crate 版本绑定 |
| `deps.cirru` | `:calcit-version \|x.y.z` | 必须与 `calcit --version` 输出一致，CI 会用它校验               |

Calcit native FFI 只支持 [Rust bindings](./ffi-bindings.md) 中的 C-safe
buffer/async/blocking/resource protocol v1。Host 与模块通过 UTF-8 Cirru EDN
bytes 交换业务数据，不再要求使用同一个 rustc 或同一个 `cirru_edn`
crate build。`calcit_ffi_build_id`、`abi_version`、`edn_version` 和 Rust-layout
business methods 均已退役。

## 升级流程

### 0. 导出版本变量（所有后续步骤均复用）

```bash
CR_VER=$(calcit --version | awk '{print $NF}')
EDN_VER=$(cargo search cirru_edn --limit 1 | grep '^cirru_edn' | awk -F'"' '{print $2}')
PARSER_VER=$(cargo search cirru_parser --limit 1 | grep '^cirru_parser' | awk -F'"' '{print $2}')
echo "calcit=$CR_VER  cirru_edn=$EDN_VER  cirru_parser=$PARSER_VER"
```

也可以用 crates.io API 查询：

```bash
EDN_VER=$(curl -s 'https://crates.io/api/v1/crates/cirru_edn' \
  -H 'User-Agent: upgrade-script' \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['crate']['max_stable_version'])")
```

### 1. 更新 Cargo.toml

```bash
sed -i "s/^cirru_edn = .*/cirru_edn = \"$EDN_VER\"/" Cargo.toml
sed -i "s/^cirru_parser = .*/cirru_parser = \"$PARSER_VER\"/" Cargo.toml
```

同时将 `[package] version` bump 一个 patch 版本（若项目有版本号发布需求）。

### 2. 更新 deps.cirru

```bash
sed -i "s/:calcit-version |.*/:calcit-version |$CR_VER/" deps.cirru
```

确保与 `calcit --version` 输出完全对应。

### 3. 本地构建验证

```bash
cargo build --release
rm -rf dylibs/* && mkdir -p dylibs && cp target/release/*.* dylibs/
calcit calcit.cirru
```

三步缺一不可：构建 → 复制产物 → 运行验证。如果只更新了 `target/release/` 而未复制到 `dylibs/`，运行时仍会加载旧库。

若模块返回编译后的正则等可复用 native 对象，不要继续跨 dylib 传递 Rust `AnyRef`，也不要改成用户手动释放的整数句柄。按照 [FFI opaque resource protocol / FFI 不透明资源协议](./ffi-resource-protocol.md) 使用 buffer v1 token、generation registry 和自动 release。

验证 release dylib 的导出符号只包含固定 C ABI，并按模块实际使用的能力检查符号：

- 同步 buffer：`calcit_ffi_buffer_version`、`calcit_ffi_buffer_free`、`<method>_calcit_ffi_v1`；
- 异步 callback：`calcit_ffi_async_version`、`<method>_calcit_ffi_async_v1`；
- blocking callback：`calcit_ffi_async_version`、`calcit_ffi_buffer_free`、`<method>_calcit_ffi_blocking_v1`；
- opaque resource：`calcit_ffi_resource_version`、`calcit_ffi_resource_release_v1`。

只实现异步 callback 的模块不需要导出 buffer protocol 或
`calcit_ffi_buffer_free`。缺失当前调用所需符号时，Calcit 会直接报告期望的
C-safe symbol，不会尝试同名 Rust function。

### 4. 提交、打标签并推送

```bash
PKG_VER=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")

git add Cargo.toml Cargo.lock deps.cirru
git commit -m "chore: upgrade cirru_edn $EDN_VER, cirru_parser $PARSER_VER; bump version to $PKG_VER"
git tag "$PKG_VER"
git push origin <branch>
git push origin "$PKG_VER"
```

### 5. 创建 PR 和 Release

```bash
gh pr create \
  --title "chore: upgrade to cirru_edn $EDN_VER" \
  --body "- cirru_edn → $EDN_VER\n- cirru_parser → $PARSER_VER\n- deps.cirru calcit-version → $CR_VER"
# 或复用已有 PR，直接推送即可触发新的 CI run

gh release create "$PKG_VER" --title "$PKG_VER" \
  --notes "upgrade cirru_edn $EDN_VER, cirru_parser $PARSER_VER" \
  --target <branch>
```

### 6. 检查 CI 状态

```bash
gh pr checks <PR_NUMBER>
```

期望输出：`All checks were successful`

## 常见问题

### CI 报版本不匹配

先检查 `deps.cirru` 中 `:calcit-version` 是否与当前 `calcit --version` 一致。
这是最常见的失败原因，频繁升级 calcit 时容易被遗漏。

### dlsym failed

按顺序排查：

1. 同步方法优先检查 `calcit_ffi_buffer_version()`、`calcit_ffi_buffer_free()` 与 `<method>_calcit_ffi_v1` 是否已导出
2. async 方法检查 `calcit_ffi_async_version()` 与 `<method>_calcit_ffi_async_v1`
3. blocking 方法检查 `calcit_ffi_async_version()`、`calcit_ffi_buffer_free()` 与 `<method>_calcit_ffi_blocking_v1`
4. opaque resource 方法检查 `calcit_ffi_resource_version()` 与 `calcit_ffi_resource_release_v1`
5. 所有导出是否使用 `extern "C"` 与 `#[unsafe(no_mangle)]`（Rust 2024 edition）
6. `dylibs/` 中是否已复制最新产物（`cp target/release/*.* dylibs/`）

### amend 后需要重打 tag

```bash
git tag -d "$PKG_VER"
git tag "$PKG_VER"
git push origin "$PKG_VER" --force
```

## 查看各项目当前状态

通过 GitHub CLI 快速检查所有 FFI 项目的最新版本和 CI 状态：

```bash
for repo in calcit-lang/calcit-std calcit-lang/dylib-workflow \
            calcit-lang/calcit-fetch calcit-lang/calcit-http \
            calcit-lang/calcit-regex calcit-lang/calcit-wss \
            calcit-lang/calcit-command calcit-lang/calcit-clipboard \
            calcit-lang/calcit-wasmtime calcit-lang/calcit-fswatch \
            calcit-lang/calcit-graphviz; do
  echo "=== $repo ==="
  gh release list --repo "$repo" --limit 1
done
```
