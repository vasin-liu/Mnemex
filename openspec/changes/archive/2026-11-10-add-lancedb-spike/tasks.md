# Tasks

## 1. 依赖准入

- [x] 1.1 在 `mnemex-store` 加入 `lancedb` 0.41.0（Apache-2.0）。对照该版本 Cargo.toml 的 feature 列表，remote / cloud 一类能关则关，只留本地数据集所需项，并在 `Cargo.toml` 注释写明版本与许可证。验证：`Cargo.toml` 为精确版本 0.41.0，`cargo tree -p mnemex-store` 出现 lancedb 且不出现本 change 未准入的直接依赖（tempfile、thiserror）

## 2. 对齐键读写（TDD）

- [x] 2.1 红测试：写入一条向量后按 `chunk_id` + `content_hash` 读回相等；删除该键不影响另一条；向量长度与打开时的维度不符返回错误。确认测试失败（红）
- [x] 2.2 绿实现：`VectorStore` 的写入、按键读取、按键删除，加上 `StoreError` 的向量失败变体（D2/D3/D5）。验证 `cargo test -p mnemex-store` 通过

## 3. 补偿与重建（TDD）

- [x] 3.1 红测试：SQLite chunk 先提交且为待索引，向量写入成功后待索引位清除且索引中有该键；向量写入失败则待索引位保持且索引中无该键；重建把待索引行补上并清位，同时丢掉 SQLite 已不存在的键。确认测试失败（红）
- [x] 3.2 绿实现：补偿入口与全量重建（D4）。验证 `cargo test -p mnemex-store` 通过

## 4. 观察记录与验收

- [x] 4.1 在本 change 目录写 `spike-notes.md`：ADR-001 四项退出标准各一行，实测或阻塞原因，未跑的项不得写成通过（D6）。验证该文件覆盖全部四项
- [x] 4.2 `cargo build --workspace` 退出码 0
- [x] 4.3 `cargo test --workspace` 退出码 0，且 `mnemex-store` 测试数 > 0
- [x] 4.4 `cargo clippy --workspace` 无 error（warn 基线内）
- [x] 4.5 `cargo tree --workspace` 直接第三方仅为 rusqlite 与 lancedb 两系；`cargo tree -i mnemex-core` 无反向边
- [x] 4.6 `openspec validate add-lancedb-spike --strict` 通过
- [x] 4.7 场景覆盖核对：vector-store 三条需求的全部场景，以及 sqlite-infra「第三方依赖最小面」的依赖树场景，逐条指到测试或命令。核对表追加在本文件末尾

## 场景覆盖

| 场景 | 覆盖 |
| --- | --- |
| vector-store「写入后可按键找到」 | `crates/mnemex-store/tests/vector.rs` `upsert_then_read_roundtrips_the_alignment_key` |
| vector-store「精确删除不影响其他键」 | 同文件 `delete_one_key_leaves_the_other` |
| vector-store「空索引被 SQLite 行补齐」 | 同文件 `rebuild_fills_an_empty_index_from_sqlite_rows` |
| vector-store「重建丢弃已不在 SQLite 的键」 | 同文件 `rebuild_drops_a_key_that_sqlite_does_not_have` |
| vector-store「成功路径最后才清位」 | 同文件 `catch_up_clears_pending_only_after_the_vector_is_readable` |
| vector-store「向量失败保持待索引」 | 同文件 `catch_up_keeps_pending_when_the_vector_write_fails` |
| vector-store「重建补上未追平的行」 | 同文件 `rebuild_fills_an_empty_index_from_sqlite_rows`（补齐后待索引位为 0） |
| sqlite-infra「依赖树检查」 | `cargo tree -p mnemex-store --depth 1`：直接第三方是 `rusqlite`，以及 lancedb 系的 `lancedb`、`arrow-array`、`arrow-schema`、`futures`、`tokio`。`tempfile` 与 `thiserror` 只出现在 lancedb 的传递闭包里，不是直接依赖。`cargo tree -p mnemex-core` 没有下游 crate，`cargo tree -i mnemex-core` 只显示 core 被 store 等使用 |

## Workflow follow-up

- 人工审阅四份工件后执行 `/opsx-apply`。
- apply 完成、验收全绿后归档：`openspec archive add-lancedb-spike --yes`（本 change 有 MODIFIED 需求，归档会替换 sqlite-infra 的「第三方依赖最小面」）。
