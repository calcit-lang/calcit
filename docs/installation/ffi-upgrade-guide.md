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

## 两个关键版本号

每个 FFI 项目需要同步维护两处版本号：

| 文件         | 字段                      | 说明                                                        |
| ------------ | ------------------------- | ----------------------------------------------------------- |
| `Cargo.toml` | `cirru_edn = "x.y.z"`     | 必须与运行时 Calcit 二进制所链接的 `cirru_edn` 版本完全一致 |
| `deps.cirru` | `:calcit-version \|x.y.z` | 必须与 `calcit --version` 输出一致，CI 会用它校验               |

两者不一致都会导致 CI 失败或运行时 `dlsym failed`。

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

1. `edn_version()` 函数是否已导出
2. `#[unsafe(no_mangle)]`（Rust 2024 edition）是否存在
3. `dylibs/` 中是否已复制最新产物（`cp target/release/*.* dylibs/`）

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
