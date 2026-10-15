# 内置 core 初始化的锁顺序

## 依据

本地完整并发回归的采样表明，`has_bundled_core_source` 在持有 `PROGRAM_CODE_DATA` 读锁时
首次初始化 `BUNDLED_CORE`。内置 Snapshot 的 schema 解析经类型名解析回查同一程序数据，
当 writer 已排队时，递归读锁可能与 writer preference 形成等待环。相关实现与 main 一致，
不是迁移 planner 新增的路径。

## 修复边界

先完成既有 LazyLock 初始化，再取得全局程序读锁进行原有源码比较。缓存内容、初始化次数、
类型来源证明与拒绝被替换 core 谓词的条件均不变；不增加锁、缓存、公共 API 或串行化 CI。

## 回归方式

在隔离子进程中保证 LazyLock 为冷状态，给 schema 查找回调加入非阻塞 `try_write` 断言，
精确检查初始化期间没有持有程序锁。有锁时立即失败而非依赖调度偶然复现死锁；子进程另有
30 秒终止保护。计数仅为确认回调确实执行的测试局部断言，不是质量统计或用户门槛。

同一回归还验证空程序不获得 bundled 证明、完整内置源码获得证明、替换 core 谓词后证明失效。
这属于内部锁与进程状态 invariant，稳定 Calcit 程序无法控制冷 LazyLock 和队列中的 writer，
因此放在 Rust，语言行为仍由既有 Calcit 测试验收。

修复前使用原 main 实现，回归在 0.04 秒内因 `WouldBlock` 失败，未挂起或改写测试预期。
修复后同一测试通过，默认 feature 的并发库回归 1016 项通过（保留原有一个显式忽略的计时
benchmark），其中包含 fuzz 和此前卡住的初始化测试。继续执行完整默认并发回归及远端 CI，
不以此前串行隔离替代并发验证。
