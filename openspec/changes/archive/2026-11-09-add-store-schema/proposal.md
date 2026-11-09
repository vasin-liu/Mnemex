# Proposal

## Why

SQLite 地基（`add-sqlite-infra`）只有打开与 FTS5 验证，`mnemex-core` 的领域类型还没有落库的地方——四张表（knowledge_objects / knowledge_events / chunks / knowledge_edge）是 spike 的直接前置（ADR-002 补偿流要对着 chunks 对齐键跑）、事件溯源不变式的第一次真实兑现（追加 + 回放推导）、以及一切检索面的挂载点。探索结论（中切）：表结构 + 迁移 + KO 持久化 + 事件追加与回放一起落地；补偿流实现、edge 递归查询、FTS 内容表随各自后续 change。

## What Changes

- **手写迁移机制**：`PRAGMA user_version` 步进的有序迁移列表，逐条事务内应用，打开时幂等执行；库版本超前于程序已知版本时拒绝打开（未知 schema 不猜）。零新依赖。
- **v1 四表 DDL**（方案 A 稀疏列）：`knowledge_objects` 单表（公共列 + 五类型特有列，NULL 保持缺省语义）；`knowledge_events` append-only（自增 seq，外键指向 KO，Feedback 的 useful 单列）；`chunks`（chunk_id + content_hash 对齐键 + pending 待索引位，ADR-002）；`knowledge_edge`（src/dst/类型，索引齐备，查询面留 retrieval change）。
- **KO 行持久化**：`StoredKo` 求和类型 + `insert_ko` / `read_ko`，五类型经稀疏列无损往返；mapper 手写（BLOB↔不透明 newtype、枚举↔INTEGER 常量），**不引 serde**。
- **事件追加与回放**：`append_event` 只增不删不更；`sync_usage_from_events` 从 Used 事件回放推导 `usage_count` / `last_used` 并写回对象行——事件溯源语义第一次从类型契约变成可运行断言。
- **打开序扩展（修改 sqlite-infra）**：open 序列在 pragmas 后加 `foreign_keys=ON`（per-connection）+ 迁移执行；FK 与事件/chunk/edge 外键配套。
- **顺手定掉 D8 遗留**：`heading_path` 落库为 `/` 级联拼接字符串（core 类型已是 String，无需改类型）。

## Capabilities

### New Capabilities

- `sqlite-schema`: SQLite 模式层的运行行为——迁移的幂等与版本防线、五类型稀疏列往返、事件 append-only 与回放推导、chunks 对齐行与 edge 行的外键约束。

### Modified Capabilities

- `sqlite-infra`: 「库级运行参数」需求扩为三项——WAL、busy_timeout、`foreign_keys=ON`（随 schema 引入外键约束，必须在连接级显式开启）；open 序列在参数之后追加迁移执行。

## Impact

- 代码：`crates/mnemex-store`（新增 `schema.rs` 迁移与 DDL、`sqlite.rs` 增 KO/事件/chunk/edge 读写 API、`StoreError` 增 Migration 变体）；`mnemex-core` 不动（无 serde、无新类型）。
- 依赖：零新增（rusqlite 已准入，继续走手写 mapper 路径）。
- 出界（留后续 change）：ADR-002 三段补偿流实现与 `VectorStore` trait 方法（`add-lancedb-spike`）；edge 递归 CTE 图查询（retrieval change）；chunks 的 FTS5 内容表与 BM25 检索（ingest/retrieval change）；KO 原地更新与归档策略（ingest change 随事件语义细化）。
