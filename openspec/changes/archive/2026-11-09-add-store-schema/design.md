# Design

## Context

`SqliteStore`（add-sqlite-infra）已提供幂等打开、WAL/busy_timeout、FTS5 验证与最小 `execute`/`query_scalar` 面；`mnemex-core` 有完整 KO 类型契约（无 serde）。本 change 把四张表落进去。约束：零新依赖；手写 mapper（BLOB↔newtype、枚举↔INTEGER 常量，规避孤儿规则——不为外部类型实现 rusqlite 的 FromSql/ToSql，只用自由函数转换）；生产路径无 unwrap/expect。

## Goals / Non-Goals

**Goals:**

- v1 四表 + 手写迁移机制（幂等、事务内步进、超前版本拒绝）
- 五类型稀疏列无损往返（方案 A）
- 事件 append-only + Used 回放推导写回——事件溯源从类型契约变成可运行断言
- chunks 对齐行（pending 位）与 edge 行，外键全部生效

**Non-Goals:**

- ADR-002 三段补偿流实现、`VectorStore` trait 方法（`add-lancedb-spike`）
- edge 递归 CTE 图遍历（retrieval change）
- chunks 的 FTS5 内容表 / BM25（ingest/retrieval change，FTS5 虚拟表挂哪个表、怎么分词都留到那时）
- KO 原地更新与归档的完整策略（ingest change 随事件语义细化；本 change 只有 insert + read）

## Decisions

### D1 表布局：单表稀疏列（方案 A，探索定稿）

`knowledge_objects` 一张表：公共列（id/kind/created_at/生命周期六字段/引用三字段）+ 五类型特有列并排（各 3–4 列，NULL 允许）。列全集 ~23 列，全可直查、全可索引。备选 B（payload 编码列，需手写 JSON/自造编码）与 C（五张表，图查询 UNION 违背统一对象空间）均否——探索已定。

### D2 枚举 ↔ INTEGER 常量映射，自由函数而非 trait impl

`Kind`(5)、`SymbolKind`(3)、`Freshness`(2)、`EventKind`(7)、`EventSource`(3) 各配 `to_int`/`from_int` 自由函数（store 侧，`from_int` 返回 `Result`，未知值报错）。不实现 rusqlite 的 `FromSql`/`ToSql`：那两个是外部 trait、类型也是外部的，孤儿规则不允许；自由函数 + `get::<i64>` 一样干净且错误路径显式。

### D3 迁移机制：user_version 步进 + 超前拒绝

```rust
const MIGRATIONS: &[&str] = &[V1_DDL];   // index+1 == 目标版本号
fn migrate(conn) -> Result<(), StoreError>
```

- 读 `PRAGMA user_version` 为当前版本 v；
- `v > MIGRATIONS.len()` → `StoreError::Migration { db_version }`（拒绝打开：旧程序不猜新库）；
- 逐条执行 `MIGRATIONS[v..]`，每条一个事务：`BEGIN; DDL; PRAGMA user_version = i+1; COMMIT`；
- 打开序（更新 sqlite-infra 的 D5）：open → WAL → busy_timeout → **foreign_keys=ON** → **migrate** → verify_fts5。

`PRAGMA foreign_keys` 是 per-connection，必须在每个连接的 open 里设，与表上的 REFERENCES 配套。迁移 DDL 本身无 FK 违规风险（建表序：objects 先于其余三表）。

### D4 事件表：append-only 的两道锁

- 结构上：`seq INTEGER PRIMARY KEY AUTOINCREMENT`（自增序号即追加序），无 UNIQUE 冲突空间；
- API 上：只暴露 `append_event(&KnowledgeEvent)`；不提供任何 update/delete 事件的接口（spec 场景"追加只增不更"）。原始 SQL 永远拦不住持连接者，契约写在 API 面。

`append_event` 用事件类型自带的构造函数输入（事件已不可变），插入列 = ko/kind/at/source/(useful)。

### D5 回放：v1 只推导事件可推导的字段

`sync_usage_from_events(ko) -> Result<(u64, Option<UnixMillis>)>`：`SELECT COUNT(*), MAX(at) FROM knowledge_events WHERE ko_id=? AND kind=Used`，写回 `knowledge_objects.usage_count`/`last_used`，返回推导值。

边界如实划清：`usage_count`/`last_used` 是事件可推导字段（回放 owns）；`model_confidence`/`freshness`/`contradiction_score` 是生产者写入列（抽取/规则/近邻计算各自的 change owns）——它们的变更同样以事件记录（Created/规则更新走各自通道），但 v1 不从事件重演它们。这条边界写进模块文档，避免"事件溯源=全字段重演"的过度解读。

### D6 StoredKo 求和类型

```rust
pub enum StoredKo { CodeSymbol(CodeSymbol), DesignDecision(DesignDecision),
                    BugRecord(BugRecord), DocumentSection(DocumentSection), CommitEvent(CommitEvent) }
```
`insert_ko(&StoredKo)` 按 variant 抽稀疏列；`read_ko(&KoId) -> Result<Option<StoredKo>>` 按 kind 列 dispatch 还原。往返一致性直接用五类型已有的 `PartialEq` 断言。备选：五个 insert/read 方法对——调用方要 match 五次，不如求和类型内聚。

### D7 chunks / edge 的最小行面

- `insert_chunk(chunk_id, ko, content_hash)`：默认 `pending=1`（待索引——补偿流写入语义：SQLite 先提交，向量侧未追平）；`set_chunk_pending(chunk_id, pending)` 供 spike 翻转。无文本列、无 FTS 表——文本与检索面归 ingest/retrieval change。
- `insert_edge(src_ko, dst_ko, edge_type: i64)`：类型值域（call/import/inherit…）的枚举归 ingest change 定义，v1 以原始 INTEGER 落库并文档声明。
- `heading_path` 定稿：`/` 级联拼接字符串（KO 模型 D8 遗留 open question 就此关闭；core 类型已是 String，无需改动）。

### D8 StoreError 扩展

新增 `Migration { db_version: u32 }` 变体（手写路径延续）；`From<rusqlite::Error>` 不变。未知枚举 INTEGER 走既有 `Sqlite` 变体包一层——复用 rusqlite 错误，不造第三个变体。

## Risks / Trade-offs

- [稀疏列 ~23 列，未来加类型要加列] → 迁移机制已在位，加列是 v2 迁移的一条 DDL；五类型字段集在 KO 模型 change 已冻结，短期无加列压力。
- [AUTOINCREMENT 事件表有写放大] → 桌面单机量级（个人仓库）远够；真到性能墙再议 rowid 策略，属 spike/性能 change。
- [`foreign_keys=ON` 改变既有库行为] → 当前无既有用户库（项目未发布），无迁移兼容负担；测试覆盖 FK 拒绝路径。
- [read_ko 的 23 列 SELECT 是宽行] → 一次性读全行比按 kind 选列少一次往返；宽行成本在桌面量级可忽略。
- [D5 的回放边界可能被误读为"事件溯源不完整"] → 模块文档写明字段所有权表；全字段重演是路线图后置项，不在 v1 冒进。

## Migration Plan

新建库直接 v1；不存在既有库需要升级（项目未发布）。回滚 = revert commit。若 spike 后表结构需调整，走 v2 迁移，不回改 v1 DDL（迁移列表不可变是机制的前提）。

## Open Questions

（无——D8 遗留的 heading_path 已定；补偿流、edge 查询、FTS 内容表的归属在 Non-Goals 里写死。）
