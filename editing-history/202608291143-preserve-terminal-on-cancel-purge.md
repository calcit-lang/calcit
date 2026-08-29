# Preserve queued terminal events during cancellation purge

Review identified that a task may already have queued Complete/Fail behind an
earlier Emit when the Emit callback requests cancellation. Purging every event
would remove that terminal while the registry still remembers its terminal
claim, leaving the task stuck in Closing.

Cancellation now performs a selective Emit-only purge. Queue metadata removes
the same event sequences and bytes while preserving any queued terminal and
its exactly-once claim. Tests cover an Emit and Complete already queued before
cancel, verify only the Emit is purged, and drain the preserved terminal to a
finished task.

review 指出取消发生时 Complete/Fail 可能已经排在较早 Emit 后面。全量 purge 会
删除 terminal，却保留 registry 的 terminal claim，导致 task 永久停在 Closing。
现在取消只按 sequence 清理 Emit，并同步更新 bytes 与 purged metrics；测试覆盖
取消前 Emit 与 Complete 已同时入队，确认 terminal 被保留并正常完成。
