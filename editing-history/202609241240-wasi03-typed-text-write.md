# WASI 0.3 类型化文本写入与 Manifest 闭环

沿 #1267 和同一 `feat/wasi-03-file-business` 分支，把 `FsPath.write-text` 接入 WASI 0.3 Component。Calcit 仍使用普通方法和 `Result<Unit,String>`；WIT descriptor、writable stream、future 和 waitable set 均留在私有 runtime helper。写入前按 UTF-8 字节数拒绝超过 4 MiB 的内容，成功后才返回 `Result.ok`；open/write/future 失败返回 `Result.err`，资源在所有正常错误路径归还。open 使用 create + truncate，因此失败可能留下截断或部分内容，明确不承诺原子替换；超限在打开文件前拒绝，不改变旧文件。

真实 Wasmtime 49 测试验证具名 preopen 的中文文本覆盖与读回、恰好 4 MiB 的分块写入、4 MiB+1 拒绝且原文件不变、缺失父目录、符号链接逃逸和读入无效 UTF-8。新增独立 WASI 0.3 Manifest 脚本，复用 native、Node JS 和 Preview 1 的同一具名 Struct/EDN 业务 fixture，验证成功输出、非法数据、缺失输入、写入失败、无 preopen，并在 CI 执行。

4 MiB+1 内容的测试暴露通用 WASM 动态字符串分配只前移 heap 指针而不增长线性内存，`str` 拼接在大输入时越界；缺陷记录为 #1317。修复分配器的溢出检查与按需 `memory.grow` 后，字符串构造成功，文件边界按契约返回 `Result.err` 而非 trap。这是业务路径发现的编译器缺陷，不增加 Calcit 表层入口或 Dynamic 逃生口。

本切片证明本地真实 host 的读-改-写；#1267/#1268 的干净 checkout、CI/review、跨后端完整验收与发布流程仍需继续，PR 暂保持 Draft。
