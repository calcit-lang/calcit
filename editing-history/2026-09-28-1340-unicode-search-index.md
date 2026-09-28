# 字符串搜索返回 Unicode 标量索引

关联 [#1453](https://github.com/calcit-lang/calcit/issues/1453)，属于 0.27.0 的独立正确性修复，不等待其他 API 改名。

## 原因与选择

`.get/.slice/.len` 已采用 Unicode 标量单位，但 `.find-index` 在 `😀a` 中搜索 `a` 时，旧 native 与实际生成的 WASM 返回 4，旧 JS runtime 返回 2。新增 Calcit definition 测试也在修复前复现了 `中a` 返回 3 而不是 1 的 native 错误。搜索结果不能传给同一个字符串的访问/切片，这是原有契约的不一致，不应通过新增 Unicode API 或机械照搬 Rust 字节索引解决。

所有入口仍使用相同 primitive：native 保留 `str::find`，只对匹配前缀计数；JS 保留 `indexOf`，用既有 surrogate 扫描方式换算索引，并保留 ASCII/BMP 快速路径；WASM 保留有界字节搜索，仅在找到时统计匹配前缀中的非 continuation bytes。WASM 的第二遍扫描限定 `j < i <= haystack_len - needle_len`，不越界、不分配、不重新求值参数。空 pattern 与不匹配路径不做前缀扫描。

核对了底层 primitive、公开 `str-find-index`、`.find-index` 以及 WASM `.includes?` / `&str:includes?` lowering。后两者只判断搜索结果是否非负，返回单位变化不会改变它们的布尔语义。split 使用单独的内部字节搜索 helper，不受影响。

## 契约与兼容边界

- 返回值保持 `Option<Number>`；内部 primitive 保持 `Number/-1`，没有新增 Dynamic 或 unsafe 路径。
- 普通 String 索引使用 Unicode 标量，组合字符不做 grapheme 合并或 normalization；显式 UTF-8 byte-count 不变。
- ASCII、首次匹配、缺失和空 pattern 行为不变；普通调用无需源码改写。依赖旧字节/UTF-16 单元偏移或应用补偿的 FFI 消费者应在编码边界自行迁移，不提供不安全的全局 fix。
- JS 的孤立 surrogate 仍沿用已有宿主行为，不变成替换字符；非法宿主字符串不在跨 backend 的 Unicode 保证范围内。
- 更新可查询的 core doc/examples、String 文档、API 角色的索引单位表、升级手册与 WASM 能力表。不新增命令、统计器、运行时依赖或公开别名。

## 测试设计

权威契约放在 `calcit.core/str-find-index :tests` 的 `scalar-search-indices`：28 条断言覆盖 ASCII、中文、emoji、组合字符、空串/空 pattern、重复与缺失 pattern、方法和前缀入口、get/slice 衔接、includes、byte-count 与参数顺序。测试使用推荐的 `Option :some/:none` 构造和普通方法；只少量直接检查内部 primitive 的哨兵契约。

已有 `scripts/check-string-unicode.mjs` 读取相同 AST，分别执行 native、生成 JS、实际生成的 WASM，不复制三套断言。它检查 stdout trace，确保 receiver 与 needle 依次各求值一次。JS-only 宿主测试只补充普通 Calcit 源码无法构造的孤立 surrogate，不新增重复的 Rust 语义测试。

已执行共享三端测试、String 与 API 角色文档代码块、三个新的 definition examples。`cargo clippy -- -D warnings` 通过。全量 `cargo test` 首次因沙箱禁止绑定本地 HTTP 端口失败，改在允许本地测试服务的权限下运行；这不是忽略用例或改变测试预期。

最终 `cargo test` 全通过；`cargo fmt --all`、`yarn compile`、`yarn check-all`（含 Agent interface、definition tests、native/JS/WASM 回归）以及 `git diff --check` 均通过。

## ASCII 路径成本

macOS arm64，Rust 1.98.1 的独立 `rustc -O` primitive 微基准和 Node 20.10.0 的 JS runtime 微基准；分别执行每轮 100,000 / 200,000 次，取七轮中位数。只测搜索/索引转换，不包含 Calcit 启动与分派；并行回归与 JIT 会带来噪声，数值不是性能承诺。

| ASCII 长度，末尾命中 | native 旧/新（ns/次） | JS 旧/新（ns/次） |
| --- | --- | --- |
| 16 | 13.05 / 19.17 | 7.96 / 26.08 |
| 256 | 173.41 / 184.80 | 16.21 / 30.08 |
| 4096 | 2472.29 / 2658.65 | 134.05 / 140.60 |

未命中的 4096 字节案例：native 2453.13 / 2457.34 ns，JS 132.01 / 127.87 ns。native/JS 不分配字符数组，未命中不额外扫描；WASM 成功命中增加一次有界前缀扫描，仍沿用现有搜索复杂度。没有为了这次修复建立新的 benchmark/统计系统。
