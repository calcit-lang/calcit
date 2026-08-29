# Async cancellation must purge accepted Emit events

When a Calcit callback cancels its own native async task, later Emit events may
already have been detached from the shared queue into the same host drain
batch. Moving the registry handle to Closing blocks new producer events, but a
queue-only purge cannot see that detached batch.

The cancellation path now purges events still held by the queue immediately
after `begin_close`. Drain also treats an Emit observed after the task entered
Closing as cancellation cleanup: it is discarded without a lifecycle error.
This is safe because the registry cannot reserve an Emit sequence after
Closing. Reserved Complete/Fail events remain deliverable for exactly-once
termination.

当 Calcit callback 在同一轮 drain 中取消 native async task 时，后续 Emit 可能
已经随 batch 从共享队列取出，普通 queue purge 无法再看到它们。取消流程现在会在
`begin_close` 后立即清理队列；drain 遇到 Closing task 的 Emit 时将其视为取消清理，
静默 discard，而 terminal 事件仍利用预留容量完成 exactly-once 收尾。
