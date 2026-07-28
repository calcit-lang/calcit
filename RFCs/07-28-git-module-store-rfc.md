# RFC: Git 模块解析与本地内容寻址存储

状态：Draft
日期：2026-07-28
关联：`docs/run/load-deps.md`

## 1. 决策

Calcit 暂不提供 registry，也不引入 workspace。模块的发布、获取与版本身份继续以 Git repository + ref/commit 为基础；模块仍以目录形态提供 `calcit.cirru`、文档与可选 Rust 动态库构建输入。

依赖声明应优先使用发布 tag；tag 是可复现安装、兼容性沟通与 CI 的最佳实践。允许使用分支名以支持开发中的模块，但分支是可变引用：每次安装或更新都应显示其解析到的具体 commit，不能把“当前分支头”当作稳定版本身份。

依赖图中同一模块只能选择一个版本：解析多个约束时统一选择最高版本，并对不同声明或无法严格满足的约束输出 warning，说明最终选中的 ref/commit 与受影响依赖。由于 namespace 是全局语义空间，不支持同项目多版本并存或 Cargo 式依赖重命名隔离。

## 2. 问题

直接把每个项目依赖完整 clone 到全局 modules 目录会造成重复体积；但单纯的 archive cache 不适合需要文档浏览、Git 状态检查、`build.sh` 和 Rust dylib 的模块。目标是在保留目录体验和 Git 路径的前提下，获得类似 pnpm 的去重。

## 3. 两层本地布局

建议将实现分为不可变 store 与项目可见链接：

```text
~/.config/calcit/store/git/<content-id>/     # 完整模块目录，按 resolved commit 内容去重
<project>/.calcit/modules/<module-name>/     # 指向 store 的符号链接或平台等价链接
```

`content-id` 至少由 canonical repository URL、resolved commit、submodule 状态与必要构建输入版本组成。tag 或分支名只用于解析；store 身份始终以其解析后的 commit 为准。链接目标是完整目录，因此 snapshot、docs、native source、已构建产物与诊断文件仍能按现有路径读取。

项目 snapshot 的 `:modules` 可继续使用模块目录路径。链接层只解决本地寻址和项目隔离，不改变模块的目录接口。

## 4. 解析与 native 模块

当前不引入 `calcit.lock`。`deps.cirru` 是唯一的依赖声明与解析来源；`caps` 每次按其 tag/branch 解析依赖图并选择统一最高版本。命令结果应记录 canonical Git URL、声明的 tag/branch、实际 resolved commit、版本选择 warning 和完整性信息，方便 CI 日志与问题排查。对于 branch 依赖，更新应明确提示其从哪个 commit 前进到哪个 commit。

Rust dylib 不是 store 的例外：其 source 与 `build.sh` 仍在模块目录中。产物必须按 target triple、Calcit ABI/version、Rust toolchain 或 build-input hash 分桶，不能跨不兼容环境复用。构建前展示将执行的脚本、来源与 hash；失败信息关联到具体 module revision。

## 5. caps 命令演进

在保留 `caps`、`outdated`、`status`、`reset` 的基础上，逐步增加：

```bash
caps add <git-url-or-org/repo>@<constraint>
caps remove <module>
caps tree
caps why <module>
caps update [module]
caps verify
```

`caps status` 区分 store 完整性、项目链接、当前 `deps.cirru` 解析结果、版本选择 warning、native 产物兼容性和用户手工修改。不可变 store 不接受直接编辑；开发依赖使用明确的 path/checkout override，而不是污染共享内容。

## 6. 非目标与验收

- 不建设中心 registry、账号、发布服务、lockfile 或语义版本多版本安装；
- 不支持 workspace，也不让多个版本进入同一 namespace 空间；
- 不把模块压缩成失去 docs/native source 的 blob。

验收：两项目共享同一 resolved revision 只保存一份内容；项目链接可独立重建；版本不一致时选择最高版本并输出可读 warning；不同 native ABI/target 不会错误复用产物；原有目录模块加载与文档命令保持兼容。
