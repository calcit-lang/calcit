# 私有 shortcut 的 JS ABI 解耦

#1916 的退场门槛要求内部实现不依赖待删除的公开名字。此前 any?/every? 已使用
私有 `&list:foldl-shortcut`，但 JS emitter 仍把它映射到公开 runtime export，
因此 Calcit source 零引用不构成完整解耦证据。

本增量仅让既有私有 Proc 直接指向对应 encoded runtime binding。boolean-shortcut
循环实现保持一份，原公开 export 作为同一 function 对象的兼容桥梁保留；函数名、
参数数量、遍历/early-exit/default 和失败传播不变。没有新表层入口、rule、checker
或 Dynamic 放宽，也没有提前删除旧入口；后续 compiler/runtime 需同源码配对发布。

现有 emitter 测试同时检查 core/user namespace 的真实编码；原 JS identity harness
只在自己的 runtime 副本移除公开桥梁，验证私有调用、桥梁身份和错误对象传播。
原附带 any?/every?/shortcut 测试在统一 runner 中 native/JS 各 7/7、WASM 4/4，
原 3 个 unsupported 排除不变。另用临时 Snapshot 与 CLI 守卫事务复制原 5 个
any?/every? AST，真实编译的 JS 在没有公开 runtime export 的副本上全部通过；
不把这个临时验证提交为新维护入口。

alpha.2 的源码与 tag 保持不变，本修复在独立工作区通过正常 PR 推进。完整
review/最新 CI/精确 main 与全部消费者逐项退场门槛仍继续。
