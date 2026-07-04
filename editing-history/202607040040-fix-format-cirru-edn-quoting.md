## fix format-cirru-edn missing quotes for strings with spaces in JS codegen

### 问题
`format-cirru-edn` 在 JS runtime 中对含空格的字符串（如 `"a b"`）输出缺少外层双引号，产生 `|a b` 而非正确的 `"|a b"`。

### 定位
`ts-src/js-cirru.mts:439` — `format_cirru_edn` 对字符串类型直接拼接 `"\ndo " + to_cirru_edn(data)`，跳过了 `writer.ts` 中 `generateLeaf` 的 `JSON.stringify` 引号回退逻辑。

### 修复
改为走 `writeCirruCode([[to_cirru_edn(data)]])` → `generateLeaf` 路径，利用已有的字符合法性检查（`isCharAllowed`）决定是否加引号。同时保持 `\ndo ... \n` 外围格式与 Rust 版本一致。

### 文件变更
- `ts-src/js-cirru.mts` — 字符串分支改用 `writeCirruCode` 格式化
- `calcit/test-edn.cirru` — 新增含空格字符串和无空格字符串的测试用例
