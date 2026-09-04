# Lease review follow-up

2026-09-05 01:30 CST

## English

- Revalidate an Issue before an existing lease is renewed, taken over, or heartbeated.
- After release-side Issue synchronization, re-read the authoritative lock ref and repair the visible mirror if a newer claim won the race.
- Refresh replacement metadata from the commit actually fetched, reject missing owner/scope fields, and fail safely when a still-present ref cannot be fetched.
- Record Wiki delivery evidence in the main Issue unconditionally and in a main-repository PR only when that PR exists.

## 中文

- 已有租约续期、过期接管或心跳前，重新校验 Issue 是否仍可认领。
- release 同步 Issue 可见状态后重新读取权威 lock ref；若新 claim 赢得竞态，则自动修复可见镜像。
- replacement 元数据取自实际 fetch 到的 commit；缺少 owner/scope 时保留 claimed 状态，ref 仍存在但 fetch 失败时安全报错。
- Wiki 交付证据始终记录在主 Issue；仅当存在主仓库 PR 时才在 PR 中重复记录。
