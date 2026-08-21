# RFC: Git 模块依赖图与项目级模块视图

状态：Draft
日期：2026-07-28
更新：2026-08-12
关联：`docs/run/load-deps.md`

## 1. 决策摘要

Calcit 继续使用 GitHub repository + Git ref 管理模块，不建设 registry，
也暂不引入 workspace、依赖别名和同一项目内的多版本 namespace 隔离。

这次演进增加四层能力：

1. 全局目录可以同时保存同一模块的多个 revision；
2. 每个项目通过 `.calcit/modules/` 中的链接获得自己的模块视图；
3. `caps` 递归读取各模块的 `deps.cirru`，解析单版本依赖图并提供
   `tree`、`why`、`status` 和 `verify`；
4. native 模块的构建产物按平台和 ABI 隔离，并生成可检查的构建回执。

依赖应优先声明为发布 tag。允许分支用于开发，但每次解析都必须 warning，
并显示最终 commit。多个可比较的版本 tag 冲突时统一选择最高版本，并显示
声明来源；不可比较的 branch、commit 或非版本 tag 不猜测“更高版本”。

整个迁移保持现有 `calcit.cirru` 的 `:modules` 内容可用。旧项目仍可从
`~/.config/calcit/modules/<repo>/` 加载，新项目则优先从自身
`.calcit/modules/<repo>/` 加载。

## 2. 目标与非目标

### 2.1 目标

- 两个项目可以使用同一模块的不同版本，互不切换对方的 checkout；
- 相同 repository + commit + native build key 在本机只安装一份；
- `caps deps.cirru` 在没有 `calcit.cirru` 的目录中也能完成递归下载；
- 依赖选择结果确定、可解释，冲突能追溯到每条父依赖；
- 项目链接只在完整解析、下载和构建成功后切换；
- native 产物不会在不同 target、Calcit ABI 或 `cirru_edn` ABI 间误用；
- 当前 `caps`、`caps outdated`、`caps upgrade`、`caps status` 和
  `caps reset` 在迁移期继续可用。

### 2.2 非目标

- 不建设中心 registry、账号或发布服务；
- 第一阶段不支持 `^1.2`、`~1.2` 等版本范围，依赖值仍是准确 Git ref；
- 不允许同一项目同时加载同一模块的两个版本；
- 不用模块别名解决 namespace 冲突；
- 不要求所有历史模块立刻增加 native manifest；
- 不在第一阶段自动删除 legacy clone 或未引用的 store 内容。

## 3. 目录模型

`~/.config/calcit/modules/` 保留给 legacy module root；新的不可变 store 使用它的
同级路径 `~/.config/calcit/module-caches/`。路径解析应集中到一个 helper，后续再
考虑 XDG 或其他平台差异。

```text
~/.config/calcit/
  modules/
    <repo>/                                # legacy clone，迁移期保留
  module-caches/
    git/<owner>/<repo>/<commit>/source/
    git/<owner>/<repo>/<commit>/realizations/<build-key>/

<project>/
  deps.cirru
  .calcit/
    caps-state.cirru
    modules/
      <repo> -> ~/.config/calcit/module-caches/.../source/
      <native-repo> -> .../realizations/<build-key>/
    tmp/
```

Git tag 或 branch 只负责解析 commit；store 的 source 身份始终是 canonical Git
URL + resolved commit。这样同一 commit 被两个 tag 指向时不会重复保存。

`.calcit/caps-state.cirru` 是本次安装的诊断记录，不是 lockfile，也不参与下一次版本
选择。它至少记录：

- canonical repository URL；
- 所有请求来源及声明的 ref；
- 选中的 ref、ref 类型和 resolved commit；
- 项目链接目标；
- branch warning 和版本冲突 warning；
- native build key、回执路径与验证状态。

项目侧由 `caps` 生成或维护的内容统一放在 `.calcit/` 内，包括模块链接、状态、
临时文件、构建日志以及未来的本机 override sidecar，避免在项目根目录或模块链接
目录散落工具文件。项目根目录只保留用户维护、需要纳入版本控制的 `deps.cirru`。
`.calcit/` 整体默认应加入 `.gitignore`；其中内容是本机安装状态，不能作为跨机器
依赖身份。

### 3.1 模块目录名冲突

现有 `:modules` 使用 repository basename，例如 `memof/`。因此第一阶段继续用
basename 建立项目链接。如果依赖图里出现 `owner-a/utils` 与 `owner-b/utils`，
`caps` 必须报错并列出两者，不能静默覆盖。依赖别名需要同时解决 namespace
身份，不在本 RFC 范围内。

### 3.2 原子切换

`caps` 按以下顺序修改本机状态：

1. 在 store 的 `~/.config/calcit/module-caches/tmp/` 下创建临时目录，解析 ref、下载 source（与 store 同文件系统，保证 rename 原子）；项目侧临时 project view 写入 `.calcit/tmp/`；
2. 完成递归依赖图和版本选择；
3. 完成需要的 native realization 与验证；
4. 写入临时 project view；
5. 原子替换 `.calcit/modules/` 中的项目链接和 `.calcit/caps-state.cirru`。

任何一步失败都保留项目原来的链接。下载完成但未被链接的 store 内容可以留给
后续安装复用，由未来的 `caps clean` 回收。

Windows 上优先使用 directory junction；不支持链接时允许显式 copy fallback，
并在 `caps status` 中标记为非共享副本，不能假装已经去重。

## 4. `deps.cirru` 协议

保持当前字符串 map 兼容，先不引入复杂 constraint 对象：

```cirru
{}
  :version |0.16.67
  :calcit-version |0.13.10
  :dependencies $ {}
    |Respo/respo-ui.calcit |0.7.2
    |calcit-lang/calcit.std |0.2.15
```

字段职责：

- `:version`：当前项目或模块自身的发布版本，由 `caps version` 管理；
- `:calcit-version`：期望使用的 Calcit 工具链版本；
- `:dependencies`：repository 到准确 Git tag、branch 或 commit 的映射。

依赖 key 继续要求 canonical `owner/repo`。GitHub URL 仅作为 `caps add` 输入，
写回文件前必须规范化。

### 4.1 项目版本迁移

当前项目版本在 `calcit.cirru :version`。迁移时不能立即删除该字段：

1. `deps.cirru :version` 存在时，它是发布工具的权威值；
2. 只有 `calcit.cirru :version` 时，`caps version get` 读取旧值并提示迁移；
3. `caps version set/bump` 只写入 `deps.cirru`，不隐式改写机器生成的 snapshot；
   迁移期 snapshot `:version` 继续作为旧版 `calcit` 的兼容字段，允许暂时不同；
4. `calcit config set version` 先保留，但提示改用 `caps version set`；
5. 等生态完成迁移后，再让 snapshot 的 `:version` 变为可选并停止写入镜像。

建议命令：

```bash
caps version get
caps version set 0.16.68
caps version bump patch
```

`caps version` 必须验证 SemVer。安装 tag 时，如果模块的 `deps.cirru :version`
与 tag（允许 tag 带一个 `v` 前缀）不一致，`caps verify` 报错。branch 模块只做
warning，因为 branch 没有稳定发布版本身份。

### 4.2 standalone `deps.cirru`

位置参数继续是依赖文件：

```bash
caps ./fixtures/demo/deps.cirru
caps /tmp/download-only/deps.cirru tree
```

项目根目录始终取该文件的父目录，项目视图写到同目录的 `.calcit/modules/`。
这个流程不得要求同目录存在 `calcit.cirru`、`package.json` 或 Git repository。
如果只想预览，`caps <file> tree --resolve` 可以解析远端但不创建项目链接。

## 5. ref 分类和 warning

不能只根据字符串长相判断 branch 或 tag。`caps` 获取远端 refs 后按以下顺序
分类：

1. 精确匹配 `refs/tags/<ref>`：tag；
2. 精确匹配 `refs/heads/<ref>`：branch；
3. 完整 commit hash：commit；
4. 都不匹配：错误。

如果远端同时存在同名 tag 和 branch，拒绝并要求用户明确修改命名；Git 自己的
模糊 ref 选择不能成为包管理语义。

warning 分级：

- SemVer tag：正常推荐路径；
- 非 SemVer tag：可复现，但无法参与“选择最高版本”，给提示；
- branch：每次安装 warning，并显示 branch -> commit；
- commit：可复现，但缺少发布版本语义，给提示。

`--ci` 不隐藏 warning。未来可增加 `--deny-branch`，让发布 CI 把 branch 依赖
升级为错误。

## 6. 递归解析与单版本选择

### 6.1 解析过程

`caps` 维护带 provenance 的请求集合：

```text
Request {
  repository,
  requested_ref,
  requested_by,   # root 或 owner/repo@resolved-ref
}
```

解析采用 fixpoint，而不是“下载时顺手递归”：

1. 读取 root `deps.cirru`，产生第一批 requests；
2. 为每个 repository 解析 refs 并选择当前 revision；
3. 读取选中 revision 的 `deps.cirru`，加入它的 requests；
4. 如果新请求改变了某模块的选择，撤销旧 revision 贡献的传递请求，再展开新
   revision；
5. 重复直到选择和边集合都不再变化；
6. 检测 dependency cycle，`tree` 展示 cycle 标记但安装不重复展开；
7. graph 完整后才进入 native build 和项目链接阶段。

模块没有 `deps.cirru` 时按空依赖处理并给兼容性提示。文件存在但无法解析，或
`:dependencies` 类型错误时必须失败，不能静默当作空依赖。

### 6.2 冲突规则

同一 repository 在项目内只能有一个选择：

- 所有请求都是可解析 SemVer 的 tag：选择最高 SemVer；
- 多个请求指向同一 commit：合并为一个选择；
- 相同 branch 名：选择该 branch 当前远端 commit，并 warning；
- tag 与 branch 混合时，仍在所有请求中选择最高 SemVer tag 并给强 warning；
  发布 tag 比可变 branch 更适合作为现有项目的兼容裁决，根项目的较低 tag 也
  不压过传递依赖明确请求的较高 tag；
- 不同 branch、commit 与其他 ref、或多个不可比较 tag：不能定义“更高”。如果
  root 有直接声明，使用 root 声明并给强 warning；没有 root 直接声明则报错，
  要求 root 在 `deps.cirru` 中显式裁决。

选择较高 SemVer tag 时，即使能自动继续，也必须打印类似：

```text
warning: selected Respo/respo.calcit@0.16.67
  requested 0.16.65 by Cumulo/cumulo-reel.calcit@0.6.7
  requested 0.16.67 by root
```

`caps --strict` 可把所有版本提升和不可复现 ref warning 变为错误，适合对依赖
漂移敏感的 CI。第一阶段不自动寻找远端“最新版本”；最高版本只在依赖图实际
请求的版本集合中选择。

### 6.3 确定性

- repository、边、warning 和 tree 子节点统一排序后输出；
- SemVer 比较使用标准 precedence，不用发布时间；
- build metadata 不参与 SemVer precedence，相同 precedence 但不同 tag 指向不同
  commit 时视为不可裁决冲突；
- 解析结果中的每个 ref 都保存 resolved commit；
- 网络并发只能改变耗时，不能改变选择或输出顺序。

## 7. `caps tree` 与命令演进

在保留已有命令的基础上增加：

```bash
caps [deps.cirru]                         # resolve + install + link
caps [deps.cirru] add owner/repo@0.1.2
caps [deps.cirru] remove owner/repo
caps [deps.cirru] tree
caps [deps.cirru] why owner/repo
caps [deps.cirru] update [owner/repo]
caps [deps.cirru] verify
caps [deps.cirru] status
```

兼容规则：

- 现有 `caps add owner/repo --version 0.1.2` 继续工作；
- 现有 `upgrade` 先作为 `update` 的别名，不立即删除；
- 现有 `download owner/repo@ref` 可以内部构造临时 root graph，默认也递归；
- `reset` 只处理 legacy clone 或显式 path/checkout override。不可变 store 不做
  `git reset --hard`，项目链接损坏时由 `caps` 重建。

`caps tree` 默认展示实际选择和来源，而不只是 root 声明：

```text
root
├─ Respo/reel.calcit@0.6.7
│  └─ Respo/respo.calcit@0.16.67  ↑ requested 0.16.65
└─ Respo/respo.calcit@0.16.67
```

建议同时支持：

- `--depth <n>`：限制展示深度；
- `--all`：重复展示共享子图，否则使用 `(*)` 引用；
- `--format json`：供 Agent 和 CI 读取，stdout 必须是单个 JSON；
- `--offline`：只使用已经解析进 store 的 refs；
- `--resolve`：没有安装时允许访问远端完成预览。

`caps why <module>` 为每个 root dependency 输出到该模块的一条最短路径，并
列出全部直接版本请求和最终选择理由。稠密依赖图不枚举所有简单路径，避免输出
组合爆炸。

## 8. `calcit` 的模块查找兼容层

目前多个命令各自拼接 `~/.config/calcit/modules/`。实现项目视图前先把它们
收敛到共享 resolver，至少覆盖运行、query、config、call-graph、docs 和 wasm
内部验证路径。

对于 snapshot 中的非相对模块路径，例如 `memof/`，查找顺序是：

1. `<snapshot-dir>/.calcit/modules/memof/`；
2. `~/.config/calcit/modules/memof/`（legacy fallback）。

`./util.cirru` 这类明确相对路径仍只相对 snapshot，不进入模块 store。项目视图
存在但单个模块链接缺失时仍允许逐项 fallback，并在 verbose/status 中提示，
这样迁移不要求一次切换所有模块。

模块源码中的 `calcit-dirname` 应解析到项目链接最终指向的完整 module view。
这保证现有 `get-dylib-path` 拼接 `dylibs/lib...` 的代码无需修改。

## 9. native 模块

### 9.1 为什么不能直接在 source store 运行 `build.sh`

现有脚本会删除并重建 `dylibs/`。如果直接在按 commit 共享的 source 中执行，
不同平台、Rust toolchain 或 Calcit ABI 的项目会相互覆盖，source 也不再不可变。

因此 native 模块分为：

- `source/`：按 commit 保存，不运行构建脚本；
- `realizations/<build-key>/`：从 source 创建的独立构建视图，项目链接指向这里。

realization 可以先使用完整工作树保证简单可靠；以后再用 reflink、hardlink 或
只读 source + artifact view 优化体积，不应在第一版提前增加复杂度。

### 9.2 build key

build key 至少包含：

- canonical repository URL 和 resolved commit；
- target triple 与 OS/architecture；
- Calcit FFI ABI version；
- `cirru_edn` version；
- Calcit version（第一阶段保守隔离）；
- `rustc -vV` 的 toolchain identity；
- build command 和显式 build environment 的 hash。

同一 build key 的 realization 构建成功后可跨项目复用。失败或未完成的临时目录
不能被项目链接引用。

### 9.3 可选 native manifest

已有 `build.sh` 继续识别。新模块建议在自己的 `deps.cirru` 增加：

```cirru
:native $ {}
  :build $ [] |sh |build.sh
  :libraries $ [] |dylibs/libcalcit_std
  :timeout-seconds 600
```

`:libraries` 使用不带平台扩展名的路径，`caps` 根据 target 补 `.so`、`.dylib`
或 `.dll`。manifest 存在时，未产生声明产物是构建失败。只有 `build.sh` 而没有
manifest 的旧模块仍构建，但给迁移提示，并至少验证 `dylibs/` 下存在当前平台
动态库。

构建脚本是来自依赖的代码执行。`caps` 执行前必须显示 module、ref、commit、
脚本路径和 hash；CI 日志也不能隐藏。后续可增加 allow-list，但第一阶段不突然
改变现有默认构建行为。

### 9.4 构建回执与验证

成功后在 realization 中写 `.calcit-native.cirru`，记录：

- build key 的全部输入；
- 开始/结束时间和 command；
- 每个产物的相对路径、大小和内容 hash；
- ABI version、`cirru_edn` version 和验证结果。

`caps verify` 检查：

1. project link 指向 state 声明的 source/realization；
2. source commit 与 store identity 一致，且没有用户修改；
3. native receipt 的 build key 与当前环境一致；
4. 声明产物存在、未通过 symlink 逃出 realization、hash 匹配；
5. 能加载动态库，存在 ABI 查询符号，且 ABI/EDN version 匹配；
6. manifest 声明的业务符号（若未来增加）存在。

动态库加载验证应放在短生命周期子进程中。错误库崩溃时只让 verifier 子进程
失败，`caps` 仍能报告 module、commit、产物路径和退出状态。

运行时继续执行 ABI 检查，不能因为 `caps verify` 已通过就跳过。加载错误应尝试
附带相邻 build receipt 的 module revision、target 和 rebuild 提示。dylib cache
应使用 canonical path 作为 key；项目链接在 watcher 运行期间切换后，提示重启
进程，不尝试卸载已加载 library。

## 10. 状态、override 与人工修改

共享 store 视为不可变。开发某个依赖时不要在 store 中直接编辑，后续使用显式
override，例如 `.calcit/overrides.cirru` 本机 sidecar 或 CLI 参数指向 path checkout。
override 的正式语法另开小 RFC，不阻塞 store 和递归解析。

在 override 落地前，`caps status` 至少区分：

- dependency graph 是否仍能解析；
- store source 是否完整、commit 是否匹配；
- project link 是否缺失或指错；
- 是否使用 legacy global clone；
- branch 与版本选择 warning；
- native realization 是否适配当前环境；
- source 或 realization 是否被手工修改。

发现 store 被修改时不自动 reset。重新安装到新的临时目录并原子替换，旧的损坏
目录留待显式 clean，可以减少误删用户内容的风险。

## 11. 渐进实现顺序

### Phase A：共享路径解析和元数据

- 集中 `calcit`/docs/query 的 module resolver；
- 项目 `.calcit/modules` 优先、legacy global fallback；
- `PackageDeps` 支持 `:version` 与可选 `:native`；
- 增加 `caps version`，只管理 `deps.cirru`；
- 远端 ref 分类，branch/non-SemVer warning；
- 保持当前单层下载行为作为兼容基线。

### Phase B：多版本 store 与项目链接

- source 以 canonical repo + commit 入 store；
- 安装完成后原子创建项目链接和 state；
- 从干净 legacy clone 导入/复用 commit，不删除旧目录；
- `caps status` 同时理解 legacy 与 project view。

### Phase C：递归依赖图

- 带 provenance 的 fixpoint resolver；
- SemVer tag 最高版本选择和不可比较冲突规则；
- `caps tree`、`caps why`、JSON 输出；
- cycle、缺失 `deps.cirru`、损坏子依赖文件的测试。

### Phase D：native realization

- build key、隔离构建目录和 receipt；
- legacy `build.sh` 兼容与可选 native manifest；
- `caps verify` 子进程加载检查；
- runtime 错误关联 receipt。

每个 phase 都可以单独发布，不能要求一次性修改所有模块。Phase A/B 稳定后再让
默认 `caps` 开启递归解析；此前可用实验 flag 在真实 Respo 依赖链验证。

## 12. 测试与验收

除仓库常规 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test`、
`yarn compile`、`yarn check-all` 和 `yarn check-agent-interface` 外，依赖管理需覆盖：

- 两项目请求同模块不同 tag，链接到不同 commit；
- 两项目请求相同 commit，store 只保存一份 source；
- 菱形依赖选择最高 SemVer，并输出全部 provenance warning；
- branch 依赖显示 resolved commit，`--strict` 失败；
- branch/tag 混合时优先 SemVer tag 并强 warning；全是不可比较 ref 且没有 root
  裁决时失败；
- 依赖环不会无限递归，`tree` 输出稳定；
- 子模块 `deps.cirru` 损坏时安装失败且旧项目链接不变；
- standalone `deps.cirru` 在空目录完成安装；
- basename 冲突不会覆盖链接；
- native build key 在 target/ABI/toolchain 改变后失效；
- native 构建中断不会留下可复用的“成功”目录；
- 产物缺失、hash 改变、错误架构、ABI 不匹配和系统依赖缺失均能定位到具体
  module revision；
- legacy `~/.config/calcit/modules/<repo>` 项目仍可运行；
- Respo 等真实项目的 compile、类型查询与 examples 回归通过。

最终验收标准：项目之间不再通过切换全局 checkout 相互影响；依赖图的每个选择
都可用 `tree/why` 解释；失败安装不破坏现有项目；native 产物不会跨不兼容环境
复用，并能在运行前通过 `caps verify` 发现主要错误。
