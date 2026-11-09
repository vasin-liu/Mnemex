# Tasks

## 1. 枚举映射与错误扩展（TDD，单元层）

- [x] 1.1 红测试：`Kind`/`SymbolKind`/`Freshness`/`EventKind`/`EventSource` 的 `to_int`/`from_int` 往返用例 + 未知 INTEGER 返回 `Err`；`StoreError::Migration` 可构造且 `Display` 可辨识；确认测试失败（红）
- [x] 1.2 绿实现：`schema.rs` 内常量映射自由函数（D2）+ `StoreError::Migration { db_version }` 变体（D8），验证 `cargo test -p mnemex-store` 通过

## 2. 迁移机制与 v1 DDL（TDD，集成层）

- [x] 2.1 红测试：新建库后 `user_version` == 迁移列表长度且四张表存在（sqlite_master 查询）；重开幂等（版本不变、数据完好）；手工把 `user_version` 顶到超前值后 open 返回 `Migration` 错误；确认测试失败（红）
- [x] 2.2 绿实现：`MIGRATIONS` 列表 + v1 四表 DDL（建表序 objects 先行，D3）+ `migrate` 事务内步进 + open 序接入（WAL → busy_timeout → foreign_keys=ON → migrate → verify_fts5），验证 `cargo test -p mnemex-store` 通过
- [x] 2.3 红绿：外键开启用例——`PRAGMA foreign_keys` 为 1；对既有 `pragmas_are_in_effect` 补断言（sqlite-infra MODIFIED 场景）

## 3. KO 行持久化（TDD，集成层）

- [x] 3.1 红测试：`StoredKo` 五类型各写入一条（覆盖 Some/None 组合：model_confidence 有值、confidence 缺省、refs 含 hash/commit/path 三态）按 id 读回逐字段相等；缺省字段读回不出现伪值；确认测试失败（红）
- [x] 3.2 绿实现：`StoredKo` 求和类型 + `insert_ko`/`read_ko` 稀疏列读写 + 未知 kind INTEGER 报错路径（D6/D1），验证 `cargo test -p mnemex-store` 通过

## 4. 事件追加与回放（TDD，集成层）

- [x] 4.1 红测试：连续追加多条事件 seq 自增且全保留；对不存在 KO 追加事件被 FK 拒绝；N 条 Used 事件（时间递增）后 `sync_usage_from_events` 写回 usage_count/last_used 并返回推导值；确认测试失败（红）
- [x] 4.2 绿实现：`append_event`（只增入口，D4）+ `sync_usage_from_events`（COUNT/MAX 写回，D5），验证 `cargo test -p mnemex-store` 通过

## 5. chunks 与 edge 行（TDD，集成层）

- [x] 5.1 红测试：插入 chunk 默认待索引、`set_chunk_pending` 翻转后读回新状态；对不存在 KO 插入 chunk/edge 被 FK 拒绝；确认测试失败（红）
- [x] 5.2 绿实现：`insert_chunk`/`set_chunk_pending`/`insert_edge`（D7），验证 `cargo test -p mnemex-store` 通过

## 6. 文档与验收

- [x] 6.1 模块文档：`schema.rs` 英文 doc（迁移列表不可变前提、heading_path `/` 级联定稿、edge_type 值域归 ingest、D5 字段所有权表），验证 `cargo doc -p mnemex-store --no-deps` 退出码 0
- [x] 6.2 `cargo build --workspace` 退出码 0
- [x] 6.3 `cargo test --workspace` 退出码 0，且 `mnemex-store` 测试数 > 0
- [x] 6.4 `cargo clippy --workspace` 无 error（warn 基线内）
- [x] 6.5 `cargo tree --workspace` 第三方仍仅 rusqlite 一系（零新依赖红线）；`cargo tree -i mnemex-core` 无反向边
- [x] 6.6 `openspec validate add-store-schema --strict` 通过
- [x] 6.7 场景覆盖核对：sqlite-schema 四需求全部场景 + sqlite-infra MODIFIED 新增场景（外键开启）逐条指到对应测试，核对表落在本文件末尾追加的小节

## Workflow follow-up

- 人工审阅四份工件后执行 `/opsx-apply`。
- apply 完成、验收全绿后归档：`openspec archive add-store-schema --yes`（注意：本 change 有 MODIFIED 需求，归档会把 sqlite-infra 主 spec 的「库级运行参数」替换为三参数版本）。

## 场景覆盖核对

| 需求 | 场景 | 测试 |
| --- | --- | --- |
| sqlite-schema / 模式迁移 | 全新建库应用 v1 | `fresh_database_applies_v1_and_reopen_keeps_rows` |
| sqlite-schema / 模式迁移 | 重开不重复应用 | `fresh_database_applies_v1_and_reopen_keeps_rows` |
| sqlite-schema / 模式迁移 | 超前版本拒绝打开 | `ahead_user_version_refuses_open` |
| sqlite-schema / KO 行持久化 | 五类型往返无损 | `five_kinds_roundtrip_including_absent_fields` |
| sqlite-schema / KO 行持久化 | 缺省语义保持 | `five_kinds_roundtrip_including_absent_fields`（BugRecord 缺省字段断言） |
| sqlite-schema / KO 行持久化 | 未知判别值报错 | `unknown_kind_integer_is_an_error` |
| sqlite-schema / 事件追加与回放 | 追加只增不更 | `events_append_only_and_usage_replays` |
| sqlite-schema / 事件追加与回放 | 回放推导写回 | `events_append_only_and_usage_replays` |
| sqlite-schema / 事件追加与回放 | 外键拒绝孤儿事件 | `events_append_only_and_usage_replays` |
| sqlite-schema / chunks 对齐行与 edge 行 | chunk 默认待索引且可翻转 | `chunks_default_pending_and_edges_require_endpoints` |
| sqlite-schema / chunks 对齐行与 edge 行 | 缺 KO 的行被拒绝 | `chunks_default_pending_and_edges_require_endpoints` |
| sqlite-infra / 库级运行参数（MODIFIED） | 外键开启生效 | `pragmas_are_in_effect` |
| 单元 / 枚举映射 | 往返与未知 INTEGER | `enum_mappings_roundtrip_and_reject_unknown` |
| 单元 / 错误扩展 | `StoreError::Migration` 的 Display | `migration_error_names_the_ahead_version` |
