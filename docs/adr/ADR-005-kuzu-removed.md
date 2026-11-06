# ADR-005：Kuzu 移除

## 状态

Accepted

## 背景

Kuzu 于 2025-10-10 归档，团队被 Apple 收购。Graphiti 等下游已迁移。v2 在 Phase 0–3 只用 SQLite，因此避开了这次返工。

## 决策

- 从选型表移除 Kuzu。Phase 1 的图是 SQLite edge 表加递归 CTE。
- 维持 SQLite-first。若 Phase 4 之后确实需要嵌入式图库，观察名单为：
  - LadybugDB（Kuzu 社区 fork，血统最接近）；
  - Grafeo（纯 Rust 嵌入式，最贴桌面栈）；
  - Lance Graph（Lance 查询层，不是独立图库）。
- 三者成熟度与形态不可互换。要引入其中任何一个，单独写 ADR，不提前承诺。

## 备选方案

- 把 Kuzu 留在候选表：上游已归档。
- 现在就从观察名单里选一个替换 SQLite：Phase 1 没有"SQLite 图不够"的证据。

## 后果

- Deep Path 与结构关联的第一版只依赖 SQLite。
- 观察名单不是路线承诺。

## 引用

- v3 §2.2
- v3 §3.4
