# 局部 async 合同与词法权限分离

## 问题与决策

Protea 升级到正式 Calcit 0.28 时，async 入口内的同步 watcher 被判定为
async callback，显式 `:async false` 也无法阻止。根因是内部
`$async-invocation` 和宿主权限共用 Fn features，CURRENT_FN_FEATURES
把外层完整集合传到宏展开后的局部函数，再合并进其 schema。

CURRENT_FN_FEATURES 只承担词法宿主能力检查。保留每个函数自身 schema
中的 async marker，但进入可继承的权限作用域时过滤这个 invocation marker。
真实 js-ffi 等权限照旧传递；没有 async marker 的普通集合复用原 Arc，
不增加普通函数预处理路径的集合复制。无需新诊断、语法或平行 API。

## 兼容边界

修复局部函数无 hint 和显式 false 两种同步声明，不将父 async 当作子 async。
局部函数自身显式 true 仍是 async，传给同步 watcher 继续严格拒绝。
函数自身 async 调用/await 合同以及真实宿主权限继承规则保持不变。

## 验证设计

行为用例放在 calcit.core/fn 的 definition-attached :tests：两个 async
外层的同步回调均可通过，独立显式 async 回调仍可传给 async 合同消费者。
正式 0.28 对该附着测试实际失败，新构建用于诊断验证，不当已发布版本。
负例加入现有 check-strict-default.mjs，明确拒绝显式 async watcher，
不新增验证 runner。继续运行现有 capability、native/JS、Rust 和 Agent gates。

关联 #1713、改写后证据统一检查 #1553，跨后端回放沿用现有入口 #1560。
外部消费者验证仅使用诊断构建，不修改正式依赖或伪称生态已发布验收。
