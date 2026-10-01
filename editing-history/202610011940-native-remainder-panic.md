# native 取余 panic 的有界修复

关联 #1557。当前 native 将两个 Number 转为 i32 后直接 `%`，零除数和 `i32::MIN % -1` 均可 panic；Calcit `try` 不能捕获 Rust unwind。用官方 0.28.0-alpha.3 执行 CLI 添加的 definition test，首先在零除数处重现 Rust panic。

改用 `checked_rem`，在这两类失败上返回既有 Type 类别的 CalcitErr；不新增诊断编号、捕获全局 panic、放宽类型或改变成功路径。通用 `rem_numbers` 同时覆盖 proc 和已有 native 调用路径。

语言可观察的错误恢复通过 `&number:rem` 的 `:tests` 验证，标注 native，包含普通 `.rem`，没有用 private native call 替代方法。Rust 测试只验证底层不 unwind 的安全性质，覆盖 12 类输入的笛卡尔积，包括非有限值、i32 极限、f64 极限和小数。

这是 issue 明确允许先行的 panic 修复，不完成整个 #1557：既有 i32 饱和转换、小数限制及 JS/WASM 差异仍由 #1559 数值规格和后续实现解决。不得凭安全测试通过关闭 issue 或宣称完整三后端一致。

验证：复现后运行 Rust 安全测试、native definition tests，再执行完整 Rust、Clippy、check-all；结果和精确提交写在 PR 中。
