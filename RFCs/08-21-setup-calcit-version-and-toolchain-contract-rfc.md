# RFC：setup-calcit 版本来源与基础工具链契约

状态：Draft

日期：2026-08-21

## 摘要

普通 Calcit 项目的编译器版本应只有一个项目级事实来源：`deps.cirru` 中的
`:calcit-version`。新的 `setup-calcit` 默认用法只负责读取这个版本并安装相应工具，不再鼓励在
workflow 中重复填写 `version`。

`version` input 暂不立即删除，因为 setup-calcit 自身测试、没有 `deps.cirru` 的临时任务和紧急
诊断仍然需要显式版本；但它降级为 fallback。若项目同时提供两个不一致的版本，Action 必须
明确失败，不能再用隐含优先级覆盖。

在这个单一版本来源基础上，setup-calcit 可以补齐缓存、校验、平台路径、结构化输出和工具选择等
基础能力，但仍保持“安装工具”的单一职责。依赖安装、类型门禁和项目测试继续由显式 CI step
执行。

`calcit-lang/setup-cr` 保留为旧 workflow 的兼容入口。GitHub Actions 不会为 Action 仓库改名
提供重定向，所以不直接重命名该仓库；新项目迁移到 setup-calcit，旧项目继续使用已发布的 tag。

## 背景

当前 setup-cr README 的主示例仍然展示：

```yaml
- uses: calcit-lang/setup-cr@0.0.8
  with:
    version: "0.9.6"
```

文档又说 `deps.cirru` 中的版本优先，而当前实现实际先读 `deps.cirru`，随后用 `version` input
覆盖。这产生了三个问题：

1. 项目升级 `deps.cirru` 后可能忘记同步 workflow，CI 安装旧 CLI；
2. 使用者无法从文档可靠判断冲突时采用哪个版本；
3. 旧 CLI 可能重写新 Snapshot 的 schema 或 metadata，最终表现为格式差异、类型退化或陌生的
   CI 失败，而不是清晰的版本冲突。

2026-08 的 0.13.27 生态升级已实际出现这种失败：项目配置已经升级，workflow 中的固定版本仍
停留在旧版，旧 formatter 随后改变了新 Snapshot。问题不在 formatter 是否“足够兼容”，而在
项目同时保存了两个互相矛盾的编译器版本。

当前实现还有一些与规模增长不匹配的基础限制：

- 用正则读取版本但没有检测多个或畸形声明；
- 下载任务没有统一 await，失败聚合和完成时机不够明确；
- 安装目录固定为 `/home/runner/bin`，把实现绑定到 Ubuntu 路径；
- 没有公开 resolved version/source/tool paths 等 outputs；
- 没有复用 tool cache，也没有下载完整性校验；
- 已不再需要的 `bundler` input 和 `bundle_calcit` 下载路径仍留在 Action 中；
- `cr-wasm` 等可选工具继续逐个增加布尔 input，扩展性较弱；
- Action runtime 版本和测试矩阵需要跟随 GitHub Actions runner 演进。

## 决策一：`deps.cirru` 是正常项目的版本事实来源

推荐文档示例改为：

```yaml
- uses: actions/checkout@v4
- uses: calcit-lang/setup-calcit@v1
```

对应项目配置：

```cirru.no-check
{} $ :calcit-version |0.13.27
```

README、Calcit 安装文档、模块模板和 workflow 模板都应优先展示无 `version` input 的形式。
显式 input 移到“无项目文件的任务与故障诊断”小节，不再作为 quick start。

### 确定的解析规则

1. 默认读取仓库根目录的 `deps.cirru`；
2. 所选 `deps-file` 缺失、或文件中没有 `:calcit-version` 时，才允许使用显式 `version`；
3. 出现多个声明或任一声明不是合法 SemVer 时，以 `E_SETUP_VERSION_INVALID` 失败，绝不回退
   到 `version` input；
4. 找到且只找到一个合法 `:calcit-version` 时，以它为项目版本；
5. 同时传入相同的 `version` 时允许执行，但 summary 标为 redundant；
6. 两个合法来源不一致时以 `E_SETUP_VERSION_CONFLICT` 失败，并同时打印两个来源和值；
7. 两者都不存在时失败，并给出创建 `deps.cirru` 的首选修复方式。

这里不再定义“谁的优先级更高”。存在冲突就代表项目状态不自洽，应先修复项目，而不是猜测
维护者意图。

### 多目录项目

增加可选的 `deps-file` input，默认值为 `deps.cirru`。它只用于 monorepo 或非根目录项目：

```yaml
- uses: calcit-lang/setup-calcit@v1
  with:
    deps-file: examples/browser/deps.cirru
```

路径必须位于 checkout workspace 内。缺失的所选文件等同于没有项目声明：只能使用显式
`version`，没有该 input 时以 `E_SETUP_VERSION_MISSING` 失败；多个或畸形版本声明均以
`E_SETUP_VERSION_INVALID` 失败且不能回退。Action 不递归搜索“最像项目”的文件，避免在
monorepo 中安装偶然找到的版本。

## 决策二：扩充基础能力，但不接管项目 CI

### 工具选择

`calcit` 和 `caps` 保持默认安装。Calcit release 只发布 `calcit`，不再构建第二个 `cr` artifact。
Action 在安装目录创建相对的 `cr -> calcit` 链接，供旧 workflow 的 `run: cr ...` 继续使用；本地安装只提供
`calcit`。`tools` input 继续用可枚举形式取代不断增加的布尔项：

```yaml
with:
  tools: calcit,caps,cr-wasm
```

兼容输入中的 `cr` 被规范化为 `calcit`；同时请求二者属于重复项并在下载前失败。输出 `tools` 也只列出
规范化名称，避免将兼容链接误当作第二个已下载工具。

`bundle_calcit` 已经不再需要，不进入 `tools`，对应 `bundler` input 和下载分支直接移除。
兼容期只需继续接受 `cr-wasm` 布尔 input；转换后得到唯一的 requested tool set。未知工具、
重复项或目标版本没有对应 artifact 时，在下载前失败。

setup-calcit 不增加 `run-tests`、`run-quality`、`install-modules` 等 input。以下步骤必须继续显式
出现在 workflow 中：

```bash
caps --ci
calcit calcit.cirru --check-only
calcit calcit.cirru analyze quality
```

这样 Action 升级不会悄悄改变项目测试范围，失败日志也能清楚区分安装、依赖、类型和运行测试。

### 缓存和安装路径

- 使用 GitHub tool cache 按 `tool/version/platform/arch` 查找和保存工具；
- 临时下载使用 runner 提供的 temp 目录，不写死 `/home/runner`；
- 最终通过 `core.addPath` 暴露缓存目录；
- 所有下载 Promise 必须 await，任一失败后不报告安装成功；
- 相同 job 内重复调用时复用缓存并保持幂等。

平台和架构必须进入 release artifact lookup。MVP 只声明并测试实际发布了二进制的组合；不把
Ubuntu 上的裸文件名假装成跨平台协议。

### 完整性与版本自检

短期至少在安装后执行轻量版本自检，确认 `calcit` 输出的版本与 resolved version 一致。中期由
Calcit release 发布机器可读 manifest，包含：

- release version；
- tool name；
- platform 与 architecture；
- asset name；
- SHA-256；
- 可选的 Snapshot/capability schema version。

Action 先验证 checksum，再加入 PATH。下载到了 HTML 错误页、旧缓存或命名错误的 artifact 时，
应在 setup 阶段失败。

### Outputs 与 Job Summary

稳定 outputs：

| output | 含义 |
| --- | --- |
| `version` | 最终解析出的 Calcit 版本 |
| `version-source` | `deps-file` 或 `input` |
| `deps-file` | 实际读取的项目文件路径；无则为空 |
| `tools` | 成功安装的规范化工具列表 |
| `cache-hit` | 是否全部来自 tool cache |

Job Summary 只记录版本来源、工具、平台、缓存命中和自检结果，不输出 token、下载 header 或其他
环境敏感信息。

## 文档迁移

1. setup-calcit README quick start 删除显式 `version`，并说明 setup-cr 的 legacy 兼容边界；
2. Calcit README 将 `:calcit-version` 从“CI hint”提升为项目工具链版本；
3. 新 workflow 模板统一引用 setup-calcit release，不复制 Calcit 版本；
4. 显式版本用法放入 advanced usage，并说明不应与 `deps.cirru` 冲突；
5. 错误文案修正历史拼写 `calcit-verison`，并给出实际读取路径。

Action 在 `@v1` 内保持兼容：优先下载 `calcit`，对旧 release 回退 `cr` 并暴露同名 `calcit` 命令；对新 release 则创建 `cr` 链接。安全要求更高的仓库仍可固定 commit SHA。

## 实施阶段

### Phase 0：文档与冲突检测

- README 默认示例改为从 `deps.cirru` 读取；
- 明确解析规则；
- 两个版本不一致时失败；
- 加入版本解析、默认或显式 `deps-file` 缺失、无声明、重复、畸形 SemVer、冲突的单元测试；
  缺失文件只在给出 input 时回退，重复和畸形声明均断言 `E_SETUP_VERSION_INVALID`，即使
  workflow 同时给出 `version` input 也不能回退。

### Phase 1：可靠安装

- await 全部下载；
- 使用 runner temp 和 tool cache；
- 增加 resolved outputs、summary 和安装后版本自检；
- 将 Action runtime 升到 GitHub 当前支持版本。

### Phase 2：工具与平台矩阵

- 引入 `deps-file`、`tools`；
- 保持 `cr-wasm` input 的兼容转换与 deprecation 提示，移除已废弃的 `bundler`；
- 对实际支持的 OS/architecture 运行 self-test；
- 缺失 release artifact 时给出结构化错误。

### Phase 3：release manifest

- Calcit release 产生 checksums/manifest；
- setup-calcit 校验下载完整性和 capability metadata；
- 缓存 key 纳入 manifest/schema version。

## 验收标准

1. 正常项目的 workflow 不填写 Calcit 版本，只改 `deps.cirru` 即可完成升级。
2. `deps.cirru` 与 input 冲突时，Action 在下载前失败并展示两个来源。
3. 安装日志能证明最终版本、来源、工具、平台和缓存状态。
4. 任一工具下载或自检失败时 Action 必须失败，不能留下部分成功状态。
5. setup-calcit 自身测试覆盖默认或显式缺失 deps、合法 deps、畸形 deps、重复声明、冲突和
   artifact 缺失。
6. Action 不隐式运行 `caps`、formatter、类型门禁或业务测试。
7. 支持的平台都不依赖硬编码 `/home/runner` 路径。

## 非目标

- 让 setup-calcit 成为新的包管理器；
- 自动修改 `deps.cirru`；
- 替项目选择“最新”版本；
- 隐式运行项目 CI；
- 通过兼容 formatter 掩盖 CLI/Snapshot 版本冲突。

## 相关资料

- `README.md`
- `docs/run/library-quality.md`
- `RFCs/07-28-git-module-store-rfc.md`
- `RFCs/08-21-type-quality-ci-adoption-rfc.md`
