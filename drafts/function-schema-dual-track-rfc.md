# 函数 schema 现状说明（已按当前实现收敛）

## 状态

这个话题已经不再是“未来 RFC”，而是当前实现约定。

为避免继续传播旧语法，本文件只保留**当前可直接照抄**的写法，不再保留早期过渡方案。

## 当前结论

- 顶层 `defn` / `defmacro` 的函数签名信息，优先写在 `CodeEntry.:schema`；
- `:schema` 是普通数据，不再使用 `quote` 包裹；
- snapshot/compact 文件中的 canonical 写法是 wrapped `:: :fn` / `:: :macro`；
- 顶层函数 schema 不再推荐在 payload 里写 `:kind :fn`；
- 局部函数、匿名函数、临时闭包没有独立 `CodeEntry` 时，仍可保留 `hint-fn`；
- 历史上的 body-hint 返回值声明、异步 hint 旧写法、quoted schema 包裹写法，都不应继续作为新文档示例。

## `CodeEntry` 当前推荐形态

```cirru
%{} :CodeEntry
  :doc |...
  :code $ quote ...
  :examples $ []
  :schema $ :: :fn
    {}
      :generics $ [] 'T 'U
      :args $ [] 'T :number
      :rest :number
      :return $ :: :tuple :ok 'U
      :where $ []
        :: 'Eq 'T
```

说明：

- `:schema` 缺失时，工具链仍可能回退到旧的 body 提示提取；
- 但**新内容不要再依赖这种回退**；
- 顶层 wrapped `:: :fn` 已经表达 kind，payload 内不再重复写 `:kind :fn`；
- `:name` 不属于 schema；
- `:rest :number` 仍表示“剩余参数元素类型为 `:number`”；
- 未填写的字段按动态语义处理。

## 当前类型 DSL

### 基础类型

- `:number`
- `:string`
- `:bool`
- `:dynamic`
- 泛型变量：`'T`

### 复合类型

- 列表：`:: :list 'T`
- 集合：`:: :set 'T`
- 映射：`:: :map 'K 'V`
- 元组/变体：`:: :tuple :tag 'T`
- 函数：`:: :fn $ {} ...`

注意：函数类型现在也统一成 hashmap payload，不再推荐旧的 positional 形式。

正确示例：

```cirru
:: :fn $ {}
  :generics $ [] 'A 'B 'C
  :args $ [] 'A 'B
  :return 'C
```

更常见的嵌套场景：

```cirru
:: :fn $ {}
  :args $ []
    :: :fn $ {}
      :args $ [] 'A
      :return 'B
  :return 'B
```

## `:where` 约束

约束统一放在 `:where`，每条约束是一条 tuple：

```cirru
[]
  :: 'Eq 'T
  :: 'Show 'T
  :: 'Ord 'U
```

约定：

- 同一变量多条约束表示“且”；
- 顺序不影响语义；
- 若当前没有约束，直接写 `:where $ []`。

## parse 校验示例

文档里保留的 schema 示例应至少能被 `cr` 解析。

主示例：

```bash
cr demos/compact.cirru cirru parse-edn "(:: :fn ({} (:generics ([] 'T 'U)) (:args ([] 'T :number)) (:rest :number) (:return (:: :tuple :ok 'U)) (:where ([] (:: 'Eq 'T)))))"
```

运行时数据验证：

```bash
cr demos/compact.cirru eval "let ((schema (:: :fn ({} (:generics ([] 'T 'U)) (:args ([] 'T :number)) (:rest :number) (:return (:: :tuple :ok 'U)) (:where ([] (:: 'Eq 'T))))))) (println schema) (println (type-of schema)) , schema"
```

## 顶层定义与局部定义的分工

### 顶层定义

优先写 `:schema`：

```cirru
%{} :CodeEntry
  :code $ quote
    defn %err (message)
      %:: Result :err message
  :examples $ []
  :schema $ :: :fn
    {}
      :args $ [] :dynamic
      :return :tuple
```

### 局部函数

仍可使用 `hint-fn`，因为它们没有独立的 `CodeEntry`：

```cirru
fn (x)
  hint-fn $ {} (:return :number)
  inc x
```

## 不再推荐保留的旧内容

以下内容不要再出现在新文档里：

- 顶层函数继续依赖旧 body-hint 返回值声明
- 旧的异步 hint 形式
- quoted schema 包裹写法
- 顶层 schema payload 继续写 `:kind :fn`
- 旧的 positional `fn` 类型形式
- 任何把 schema 当作“未来提案”而非“当前约定”的表述

## 文档整理原则

后续若继续整理 drafts：

- 只保留当前代码还能直接对照的写法；
- 已经完成迁移的讨论，优先删除而不是保留过时阶段描述；
- 历史背景放在 `editing-history/`，不要继续堆在 drafts 里。
