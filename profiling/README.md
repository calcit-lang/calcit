# Profiling 工具说明

本目录集中放置 calcit 的 profiling 脚本：

- `profile-once.sh`：基于 `xctrace Time Profiler` 一键录制并汇总。
- `profile-summary.py`：解析 `xctrace` 导出的 XML 热点。
- `samply-once.sh`：基于 `samply` 一键录制并汇总（函数级 self-time）。
- `samply-summary.py`：解析 `.samply` 并输出热点函数占比。

## 1) xctrace 路线

### 一键执行

```bash
profiling/profile-once.sh calcit/fibo.cirru --top 20 --include 'calcit::'
```

输出：

- `.tmp-profiles/<entry>-<timestamp>.trace`
- 终端中的热点函数统计、前缀聚合和优化提示

### 仅汇总已有 trace/xml

```bash
python3 profiling/profile-summary.py --trace .tmp-profiles/fibo-xxxx.trace --top 30
python3 profiling/profile-summary.py --xml /path/to/time-profile.xml --top 30
```

常用参数：

- `--top N`：显示前 N 个热点
- `--include REGEX`：仅保留匹配符号（可重复）
- `--exclude REGEX`：排除匹配符号（可重复）
- `--keep-xml`：保留由 trace 导出的临时 XML

## 2) samply 路线（更精准函数级）

### 一键执行（debug）

```bash
profiling/samply-once.sh calcit/fibo.cirru --top 20 --collapse-hash
```

### 一键执行（release）

```bash
profiling/samply-once.sh calcit/fibo.cirru --release --top 20 --include 'calcit::|im_ternary_tree|alloc::'
```

输出：

- `.tmp-profiles/<entry>-<mode>-<timestamp>.samply`
- 终端中的函数级 self-time 热点（含占比）

### 仅汇总已有 `.samply`

```bash
python3 profiling/samply-summary.py --input .tmp-profiles/fibo-debug-xxxx.samply --binary target/debug/cr --top 30
```

release 对应：

```bash
python3 profiling/samply-summary.py --input .tmp-profiles/fibo-release-xxxx.samply --binary target/release/cr --top 30
```

常用参数：

- `--thread INDEX`：指定线程，默认自动选择样本最多线程
- `--collapse-hash`：折叠 Rust 哈希后缀，便于分组
- `--image-base 0x100000000`：`atos` 符号化基址（默认值）
- `--include / --exclude`：正则过滤

## 依赖

- xctrace 路线：`xctrace`, `cargo`, `python3`
- samply 路线：`samply`, `cargo`, `python3`
- 可选（samply 地址回填）：`atos`（macOS 自带）

## 说明

- `samply-once.sh` 会先 `cargo build`，再直接对 `target/debug/cr` 或 `target/release/cr` 录制，避免把 `rustc` 编译过程混入热点。
- 临时产物位于 `.tmp-profiles/`，可按需清理。
