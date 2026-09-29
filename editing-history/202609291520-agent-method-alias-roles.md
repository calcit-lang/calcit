# 方法查询展示已证明的首选与兼容角色

## 问题

同一个具体接收者的旧方法和首选方法可能都显示 `proven`、指向同一个 core 实现。仅靠 `status` 和 definition path，Agent 无法区分“类型可调用”和“当前推荐的应用写法”，容易继续生成历史别名。另一方面，把 `:internal` 标签或名字前缀当成公开性判断，会误隐藏 Option/Result、数学函数或公开构造器。

## 处理

`query type`、`type-at`、`context` 仅对现有 `calcit fix` 显式规则中，接收者、旧新方法实现、参数、变长参数及返回类型均得到同一证明的别名标注角色。首选方法显示 `preferred`；旧方法保留并显示 `compatibility`、目标方法和对应 fix 规则。规则元数据来自同一套 fix alias 定义，不新增独立公开 API 清单或顶层命令。`open`、`ambiguous`、自定义同名方法以及不同实现的 Map `.add/.assoc` 不因拼写获得角色。

## 验证

- 查询测试覆盖 List/Map 的谓词方法与 Set 更新别名，并验证 Map `.add/.assoc/.dissoc` 不被错误标记。
- 现有 fix 规划与查询复用同一个“同实现同契约”证明，保留旧名精确查询和实现来源。
- 继续运行 Agent 查询接口、文档和完整仓库检查；首批不处理 List `.add/.append`、`.count/.len` 等不同规划器的映射，也不改变旧 preset。
