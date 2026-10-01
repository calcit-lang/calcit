# Snapshot 并发写入保护

两个 CLI 进程同时修改不同 definition 的 doc 时，原实现均成功退出，却会静默丢失其中一项修改。已有 transaction/cursor 原子替换只保护部分写盘出口，不能保护普通 edit/config 的读改写周期。

## 决策

- 将原子 staging 实现移到共享 util，普通 Snapshot 保存、format、schema/FFI metadata 与 type-slot config 同样使用它，不新增命令或另一套写盘实现。
- 在 mutation 读取 source/cursor 之前取得 `.calcit/<Snapshot 文件名>.lock` 的内核独占锁，最多等待 5 秒；`fix --apply` 必须在载入 compiled evidence 前取得锁。
- 保留锁 inode，不按文件年龄删除锁。正常结束清空持有者记录；进程中断后内核释放所有权，下次取得锁后报告残留记录。原子 staging 的内部锁同样遵守此协议。
- 提交前比较锁定时的 source bytes，自己的成功提交推进该值。首次性能试验发现重复 MD5 超过 10ms 目标，改为精确字节比较，不减弱 revision 检查，也不改公开 revision 的格式。

## 验证与边界

进程与文件系统 invariant 放在现有 Rust edit CLI 测试和共享 staging 单元测试：两进程各 20 次写入、入口共用活跃旧锁、终止等待 stdin 的 CLI 后恢复、外部内容变化拒写、多次提交与原子替换。语言语义没有变化，不新增 Calcit 测试数量指标或独立脚本。

真实 Respo Snapshot 原文件只读，性能验证在临时副本进行，使用当前未发布分支构建而非声称已发布工具链具备修复。315824 字节副本的 21 次中位数测量：原子写入基线 7.132ms，保护写入 7.715ms，额外 0.583ms。完整验证结果以 PR 为准。

Review 后锁记录读写统一使用已持锁的句柄；Unix rename 后尽力同步父目录。内部锁使用原生文件名保留文件系统的大小写别名语义。性能阈值保留 10ms，但不作为普通并行单元测试的时间门禁，使用显式 benchmark 隔离运行，避免 CI 磁盘抖动：

```bash
cargo test --lib util::atomic_write::tests::snapshot_guard_adds_less_than_ten_milliseconds_to_serial_writes -- --ignored --test-threads=1 --nocapture
```

可用 `CALCIT_WRITE_GUARD_BENCHMARK_SNAPSHOT` 选择真实 Snapshot；测试先复制源文件，不修改该原文件。

锁协调遵守协议的 CLI；旧 CLI 与外部编辑器不受内核 advisory lock 强制限制，提交前比较不等于任意外部写入的 CAS。不自动合并、自动重试业务修改或改变 Snapshot 格式。`.calcit/` 锁状态为本地工具数据，不应提交到仓库；有活跃进程时不能清理锁文件。
