# 整项目 `fix` 的 target 边界

Cumulo Reel 的默认入口为 browser，server 入口为 Node。两个入口分别通过严格检查，但整项目 `fix --preset surface-latest-v2` 会遍历所有项目定义，却沿用默认 browser target，导致 server-only 的 external-object 方法在预览阶段误报 target mismatch。

整项目改写计划没有唯一宿主 target。预处理这类计划时临时清除 active target，保留普通类型、依赖图、FFI feature 与 staged transaction 检查；结束后恢复原 target。带 `--ns` 的局部计划仍遵循所选 entry 的 target，真正的运行能力由逐入口 `--check-only` 或严格工作流验证。该修改不让 Node-only 代码在 browser 下合法运行，也不改变生成器行为。

用双 target 最小 fixture 覆盖整项目预览成功、browser 局部预览拒绝 Node-only 方法、Node 局部预览成功；现有多入口依赖缺失回归继续保留。在 Cumulo Reel draft #46 的合并 HEAD 上，使用候选编译器复测整项目 EDN 预览、原文件哈希及两个入口的严格检查。
