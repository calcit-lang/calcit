# macOS main-thread blocking FFI / macOS 主线程 blocking FFI

- On macOS, run the Calcit CLI on the process main thread so AppKit/winit event loops invoked through blocking FFI satisfy the platform contract.
- 在 macOS 上让 Calcit CLI 直接运行于进程主线程，使 blocking FFI 启动的 AppKit/winit 事件循环满足平台约束。
- Reserve a 32 MiB Mach-O main-thread stack, matching the worker stack used on other platforms and preserving preprocessing headroom.
- 为 Mach-O 主线程预留 32 MiB 栈，与其他平台的 CLI worker 一致，避免预处理递归空间退化。
- Verified with a real calcit-paint C-safe blocking-v1 window/callback/render smoke; the prior worker-thread build failed before EventLoop creation.
- 使用 calcit-paint C-safe blocking-v1 的真实窗口、回调和渲染 smoke 验证；旧 worker-thread 构建会在 EventLoop 创建前失败。
- Added a macOS CI build that checks the Mach-O `LC_MAIN` stack reservation and CLI startup.
- 新增 macOS CI 构建，持续检查 Mach-O `LC_MAIN` 栈预留与 CLI 启动。
