---
title: "结构化编辑策略（5 招）"
summary: "最小化手写代码的编辑策略：cp 复制子树、mv 移动定义、wrap 包裹、raise 提升、rewrite 重排"
scope: "core"
kind: "guide"
category: "run"
aliases:
  - "structural edit"
  - "cp strategy"
  - "wrap strategy"
  - "raise strategy"
  - "rewrite strategy"
entry_for:
  - "cr tree cp"
  - "cr edit mv"
  - "cr tree wrap"
  - "cr tree raise"
  - "cr tree rewrite"
---

# 结构化编辑策略（常用 5 招）

下面是"尽量不手写大段代码"的编辑策略，按风险从低到高使用。

## 1) `cp`：复制现有子树，减少手输

```bash
cr tree cp app.main/demo --from '3.2' -p '4' --at after
```

- 含义：把路径 `3.2` 的子树复制到 `4` 后面。
- 适合：先复用旧逻辑，再做小改。

## 2) `mv`：移动/重命名定义

```bash
cr edit mv app.main/old-name app.main/new-name
```

- 含义：定义级重命名或迁移。
- 适合：整理命名或模块边界。

## 3) `wrap`：给目标套一层结构

```bash
cr tree wrap app.main/demo -p '5.2' -e 'when cond self'
```

- 含义：把原节点作为 `self` 嵌入新结构。
- 适合：快速加 guard、日志、转换壳。

## 4) `raise`：提升子表达式，去掉中间壳

```bash
cr tree raise app.main/demo -p '5.2.1'
```

- 含义：用指定子节点替换其父节点。
- 适合：去掉多余 `let/when/pipe` 包裹层。

## 5) `rewrite`：引用原节点做结构重排

```bash
cr tree rewrite app.main/demo -p '5.2' --with self=. -e '-> self normalize emit'
```

- 含义：在新模板中引用原节点（`.`）。
- 适合：复杂重构但希望保持局部语义。

> 实战建议：先 `search-replace/cp/wrap`，再用 `rewrite`；每步后 `tree show` 复核。
