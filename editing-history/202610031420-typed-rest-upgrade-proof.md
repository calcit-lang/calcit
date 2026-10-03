# 升级规划复用既有 rest 调用证明

关联 #1694、#1539、#1553。真实 Command 升级暴露合法变长调用被固定 spread 改写规则一律列为待审的问题。

本次只复用编译器已有的固定参数加单一末尾 rest 投影与通用类型证明。可独立证明的变长调用已经是合法源码，不生成 replacement，也不改成固定 arity；保持 head、参数和 spread 的求值语义。固定 literal spread 的原子改写不变。

未知 callee、未证明类型、缺少固定参数、optional、trait obligations 和不稳定 macro/source 映射仍待审或报错。不新增 FFI 名称白名单、proof 规则、诊断或 CLI 入口。注册的 native proc 缺少类型合同与 core str 的其他传播证明问题仍需独立核实，不能据此宣称整个 Command 升级已完成。

独立 fix fixture 的 definition :tests 表达非空/空 rest 调用语义；Rust 验证 envelope、无写入、重复无建议及保留错误/开放边界。公共 fix fixture 不增加定义或改变预期。文档说明“无须改写”与“可安全展开”是不同结果。

初始用例经前缀 count 引入 core/count 的 E_FN_RETURN_UNPROVEN 审计缺口；完整回归未因此放宽。当前示例使用已有具体 List 的 canonical .len 语义，count 的泛型 fallback 与 core str 分开记录，不能把这里的成功当作那些开放实现已证明。
