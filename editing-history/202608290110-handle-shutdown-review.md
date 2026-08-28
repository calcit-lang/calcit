# 完善关闭中断路径 / Complete shutdown interruption paths

## 中文

- 按 condvar 谓词协议在持有 wake mutex 时记录 shutdown 并通知，消除检查状态与进入等待之间的丢失唤醒窗口。
- Native evaluator 低频检查 shutdown 请求，覆盖普通求值与尾递归执行；`on-control-c` 回调在 host thread 运行时临时暂停该检查。
- Watch 在收到文件事件后重新检查关闭状态；reload 与 codegen 在阶段之间检查，且无 timeout 的 codegen 也由可中断的 worker 执行。
- Once-mode 运行出错或被中断后不再提前跳过 async runtime 清理，而是先完成有界取消与回收，再返回原始错误。

## English

- Record shutdown and notify while holding the condition-variable predicate mutex, closing the lost-wakeup window between checking state and waiting.
- Poll shutdown cheaply during native evaluation, including tail recursion, while temporarily suppressing interruption for the host-thread `on-control-c` callback.
- Recheck shutdown after watch events and between reload/codegen phases; timeout-free codegen now also runs behind an interruptible worker boundary.
- Once-mode failures and interruptions no longer bypass async-runtime cleanup: bounded cancellation and reclamation finish before the original error returns.
