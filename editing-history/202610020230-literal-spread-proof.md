# 固定 literal spread 的证明与安全写回

复用现有 fix suggestion、source-expression trace、来源稳定性判断与 staged transaction，增加显式选择的 spread-call-proof-v1，不增加顶层命令或默认 preset。

源码有 List 字面量并不构成完整证明。先确认唯一 source call 的预处理结果仍是 CallSpread，List 的真实 head 是内建构造器，固定 callable 的 arity 与展开后实参数量相同，再复用编译器 prove_with_bindings 检查每项。未知、开放、optional/rest 与尚未证明的 where bounds 保持待审，不使用旧 compatibility 关系代替证明。REFACTOR_SPREAD_CALL 表示结构改写建议；已有 E_SPREAD_TYPE_MISMATCH 表示非法 spread，不能用错误编号给合法程序的规范化建议命名。

初版把所有 macro origin 拒绝，导致正常 let/do 内含副作用的实参也无法改写。实际 trace 表明类型证明完整，缺口在于把已有 core let/do 来源误当作任意宏。改为复用既有来源稳定性判断，未知用户宏仍需 review；没有增加宏白名单或放宽类型关系。保持 head 与每个实参子树原样且只出现一次，嵌套已证明改写组合成一次外层 replacement。

用户可观察的 spread/direct 参数单次求值顺序写在 core definition 的 :tests，复用现有 spread runner 在 native 与真实 JS 回放。CLI preview、不写回负例、过期 revision 与幂等性留在已有 fix_cli 的底层事务回归。此记录是开发中设计依据，不代表完整 issue 的六类迁移规则已经交付。
