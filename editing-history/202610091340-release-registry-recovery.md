# 发布中断后的 registry 配对恢复

alpha.21 首轮发布在 crate 上传后遇到 TLS 断连；第二轮的旧 `publish-crates@v1` 在读取 annotated tag 历史时访问了不存在的 `history.edges`，npm 步骤没有执行。直接重跑不能解决这项可复现的恢复缺口。

改用官方 Cargo/npm 命令，并在 registry 版本已存在时先核对 source SHA，crate 另核对 sparse index checksum 与 clean VCS metadata。已有 Release 可在同一个 workflow 从 main 指定 tag 恢复，脚本来自已验证 main，构建源码来自原 tag；两份精确 SHA 的原 main 门禁均须成功。保留全部原测试与资产构建，不创建平行的 npm-only 发布入口，不移动 tag。

发布任务串行，npm 发布前检查 channel 不回退；任何 registry 网络错误或未知 source identity 均停止。公开帮助集中在 installation/release-recovery.md。

验证覆盖版本/Release/channel 一致性、缺失或失败的精确 CI、源提交不匹配、dirty crate、registry 错误与 checksum 错误。GitHub Actions 文件另经 actionlint；实际 alpha.21 crate 和原 main CI 使用只读请求核实。语言源码与 Snapshot 未修改。
