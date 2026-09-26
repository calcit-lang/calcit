# JS 字符串索引对齐 Unicode 标量

关联 #1377；后续 WASM 遗留由 #1381 跟踪，属于 0.23.0。

## 原因与选择

0.22.1 的 JS `&str:count` 已按标量计数，但 `&str:nth`、`&str:first`、`&str:contains?`、`&str:rest` 与 `&str:slice` 仍使用 UTF-16 单元。`last |中😀` 的 guard 使用长度 2，读取索引 1 却返回高 surrogate。保留 native 契约，只修 JS primitive，不添加 source 迁移、类型断言或平行 API。

内部以无数组分配的 scalar offset 扫描换算 UTF-16 偏移；ASCII/BMP 经无 high-surrogate 的快速路径直接使用偏移。索引校验拒绝负数、小数及非有限数；越界读取返回 nil，切片截到末尾。更新旧 runtime last/butlast 导出，防止它们继续截断字符。公开 nth/get 的既有 guard 不改变。

## 验证

- 在 `calcit.core/last :tests` 增加正常与非法索引回归，通过结构化 CLI 写入。
- `scripts/check-string-unicode.mjs` 从 core Snapshot 读取同一份测试 AST，在临时副本生成入口，执行 native 与生成 JS；无重复业务断言，无新公开 CLI。接入已有 check-js-runtime 与 PR/release workflow。
- 语义覆盖 ASCII、BMP、emoji、组合字符计数、空串、越界、切片与普通方法。JS host 的 NaN/Infinity 检查放在既有 runtime 边界测试。
- 基线 0.22.1 的 JS runtime 通过 TypeScript 转译后，`&str:nth("中😀", 1)` 返回 `"\ud83d"`；修复后为完整 emoji。
- 本机 Node 20.10.0 / macOS arm64，50 万次 ASCII 末尾读取：16 字符原生下标基线 2.36ms / 新实现 11.95ms；128 字符 7.39ms / 10.43ms。仅作微基准，包含 JIT 与检查成本，不代表应用耗时保证。未加快速路径时 128 字符扫描约 191ms，因此保留快速路径；未引入缓存或指标门禁。

## 边界

这是 Unicode 标量，不是字素簇；JS FFI 孤立 surrogate 未纳入保证。WASM count/byte-count 保持已有行为，但实际执行 `&str:contains? |😀 1` 得到 true，native 为 false；读取仍按字节、rest 空串与 slice 内存边界需要另修，不能将这次 JS 修复宣传为 WASM 全面对齐。文档明确这些限制。
