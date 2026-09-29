# Ref watcher 效果命名

## 决策

`add-watch` / `remove-watch` 是修改 Ref watcher 注册表的底层 proc，参数分别为 `(Ref<T>, Tag, (T,T)->Unit)` 与 `(Ref<T>, Tag)`，都返回 Unit。重复注册和移除不存在的 key 保持原有错误。应用层新增 `add-watch!` / `remove-watch!` 两个具名 core 函数，以相同参数和返回 schema 调用原 proc；旧名保留底层兼容，不改变原错误消息或 callback 调用次数。

`!` 在这里表达显式修改 watcher 注册状态，不表示所有可能抛错或读取外部状态的函数都应加后缀。效果图继续同时识别新旧调用为 `state/watch`。现有类型/来源证据尚不足以保证任意同名用户函数或宏的源码改写安全，因此本批不新增机械 rename/fix，也不删除旧 proc。后续 #1457/#1458 的迁移需单独证明来源、作用次数、求值顺序和 attached tests/examples 的处理。

## 验证与边界

- core definition `:tests` 检查正常注册、一次回调、重复 key 错误、移除后不再通知及缺失 key 错误；`calcit/test.cirru` 的 Ref smoke 同时覆盖生成 JS。
- `query def` 的 Cirru EDN 结果显示新入口的泛型 schema、`:state :watch` 标签和旧入口兼容说明。
- native/生成 JS 是本批实际验证的执行路径；不因命名变化新增 WASM/WASI watcher 能力，未支持目标继续明确 unsupported。
- 旧调用仍在主测试中运行，以验证兼容窗口。跨仓库 js-ffi/std 的效果命名和真实消费者迁移仍需独立 PR，不凭相同 `!` 后缀合并不同错误模型。
