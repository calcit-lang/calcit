# Calx cached baseline 结果

## 中文

从 clean commit `88bb5a2250ba65b0e35c4d1809e6d49a14c61623` 重新采集完整 schema v2 baseline：

- Apple M1 Pro、macOS arm64、Rust 1.97.1，`gitDirty=false`；
- debug/release 共 26 个 case，每个 case 保留 7 个 fresh-process 样本，共 182 个原始样本；
- cached Calcit callable 与 reused Calx VM 都预热 20 次并测量 100 次；
- release Calx compile total 中位数约 41–78 μs；
- release Calx hot / cached Calcit ratio 为 0.106–0.392，即有限样本约 2.6–9.5 倍；
- lookup-native ratio 为 0.009–0.150，确认旧比较夸大了微型 kernel 差距，但公平 cached 对比下 scalar 收益仍存在；
- range-sum、Fibonacci 的 one-shot crossover 仍分别为 100、10；bounded-simulation 在本次样本移到 1000；affine、polynomial 未出现；
- 大型 JSON 继续由 `.gitattributes` 标记为 `-diff linguist-generated`。

这些证据支持下一步先建立 profile-backed compile/program cache issue。VM setup 明显小于 compile，但仍需独立
profile 才能决定 VM pooling。typed buffer、peak RSS/allocation、WASM 参照与跨机器重复仍由 calx-vm #39/#50 追踪。

---

## English

Recollect the complete schema-v2 baseline from clean commit `88bb5a2250ba65b0e35c4d1809e6d49a14c61623`:

- Apple M1 Pro, macOS arm64, Rust 1.97.1, with `gitDirty=false`;
- 26 debug/release cases with seven retained fresh-process samples each, for 182 raw samples;
- 20 warm-ups and 100 measured calls for both the cached Calcit callable and reused Calx VM;
- median release Calx compile total of approximately 41–78 μs;
- a release Calx-hot/cached-Calcit ratio of 0.106–0.392, or about 2.6–9.5 times faster in the bounded sample;
- a lookup-native ratio of 0.009–0.150, confirming that the old comparison overstated tiny-kernel gaps while the fair cached comparison still retains a scalar gain;
- one-shot crossovers of 100 for range-sum and 10 for Fibonacci; bounded-simulation moves to 1000 in this sample, while affine and polynomial have none;
- the large JSON remains marked `-diff linguist-generated` through `.gitattributes`.

The evidence supports filing a profile-backed compile/program-cache issue next. VM setup is much smaller than
compilation, but VM pooling still requires separate profile evidence. Typed buffers, peak RSS/allocation, the Wasm
reference, and cross-machine repetition remain tracked by calx-vm #39/#50.
