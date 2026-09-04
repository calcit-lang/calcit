# Make host capability states explicit

2026-09-05 02:15 CST

## English

- Split grouped process, timer/date, glob/crypto, HTTP, and WebSocket capabilities where backend support differs.
- Use concrete Node adapter, browser adapter, or unavailable states instead of ambiguous cross-backend wording.
- Keep the matrix descriptive of current implementations rather than promising universal support.

## 中文

- 当 backend 支持不同，拆开 process、timer/date、glob/crypto、HTTP 与 WebSocket 能力。
- 使用明确的 Node adapter、browser adapter 或 unavailable 状态，移除跨 backend 歧义。
- 能力矩阵只描述当前实现，不承诺所有 backend 都具备同一能力。
