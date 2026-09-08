# Async invocation review follow-ups

## English

- Address the post-merge review findings from calcit#923.
- Preserve pending async results through `apply` and reject pending receivers.
- Keep the internal async marker out of public feature sets and normalize string-keyed `:async` schemas.
- Tighten definition-level hint detection and add focused regression coverage.
- Migrate `test-async-in-data` to await the nested async result before synchronous consumption.

## Validation

- `RUST_MIN_STACK=16777216 cargo test` — 1113 tests passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `yarn try-js` — emitted JavaScript and completed the real Node runtime suite.
- Static lowering, IR, WASM, literal-path, typed-method and documentation gates passed.

## Remaining after the draft checkpoint

- Wait for latest-head CI and automated review, then address any valid comments.
- Merge this follow-up before validating the js-ffi consumer tracked by calcit#910.

## 中文

- 处理 calcit#923 合并后审阅发现的问题。
- 在 `apply` 路径保留待 await 的异步结果，并拒绝未 await 的 receiver。
- 防止内部异步标记进入公开 feature 集合，并归一化字符串键形式的 `:async` schema。
- 收紧定义级 hint 检测并补充聚焦回归测试。
- 迁移 `test-async-in-data`，在同步消费嵌套 async 调用结果前显式 `js-await`。

## 验证

- `RUST_MIN_STACK=16777216 cargo test`：1113 项测试通过。
- `cargo clippy --all-targets --all-features -- -D warnings`：通过。
- `yarn try-js`：成功生成 JavaScript，并完成真实 Node 运行时测试。
- 静态 lowering、IR、WASM、literal-path、typed-method 和文档门禁均通过。

## Draft 检查点后的待办

- 等待最新 head 的 CI 与自动审阅，处理全部有效意见。
- 合并本 follow-up 后，再验证 calcit#910 跟踪的 js-ffi 消费端。
