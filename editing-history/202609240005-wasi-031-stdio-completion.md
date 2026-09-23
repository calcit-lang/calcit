# WASI 标准输出 completion 检查 / WASI output completion check

2026-09-24 00:05 +0800

## 中文

#1266 的首个输出切片会释放 `future<result<_, error-code>>`，但没有读取最终宿主结果。现在在关闭 writable stream 后同步等待 completion future：只接受已完成且为 `ok` 的结果，其余情况显式 trap；再释放临时 Canonical ABI 缓冲区和 future handle。Calcit 表层 `println` / `eprintln` / `echo` 保持原来的无 `Result` 契约，不新增 API。bindgen 的独立 ABI 回归覆盖大块输出和关闭输出端。

## English

The first #1266 output slice dropped `future<result<_, error-code>>` without reading the host completion result. After closing the writable stream, the compiler now synchronously awaits that future, accepts only a completed `ok`, traps otherwise, and releases the temporary Canonical ABI buffer and future handle. Calcit's surface `println` / `eprintln` / `echo` keep their non-`Result` contract, with no new API. An independent bindgen ABI regression covers large output and a closed output pipe.
