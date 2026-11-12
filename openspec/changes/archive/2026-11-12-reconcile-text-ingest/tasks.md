# Tasks

## 1. 扫描报告（TDD）

- [x] 1.1 红测试：临时目录里，空的 `.txt` 出现在本次见到的文本路径中且不产生章节；非 UTF-8 的 `.txt` 出现在非 UTF-8 路径中，扫描仍成功；同目录的 `.md` 仍产生章节。确认测试失败（红）
- [x] 1.2 绿实现：按 D1 让 `scan_text` 返回章节、文本相对路径和非 UTF-8 路径，分隔符保持 `/`。验证 `cargo test -p mnemex-ingest` 通过

## 2. 对账计划（TDD）

- [x] 2.1 红测试（无 IO）：相对路径消失则删除该对象；标题变化则删除旧对象标识；非 UTF-8 路径上的已有章节保留；路径还在但没有章节则删除；正文变化则删除旧 chunk、更新内容哈希并给出新 chunk；内容不变则计划为空。确认测试失败（红）
- [x] 2.2 绿实现：按 D2 在 `mnemex-ingest` 完成纯函数。`mnemex-store` 不依赖 `mnemex-ingest`。验证 `cargo test -p mnemex-ingest` 通过

## 3. 存储删除（TDD）

- [x] 3.1 红测试：用真实 SQLite，删除一条 chunk 后，被删正文里不少于 3 个字符的词不再命中，另一条仍命中；移除带 chunk、创建事件和边的文档章节后，该对象、chunk、事件和边都不存在，另一种类型的对象及其 chunk 仍在，其他对象的事件仍在；同一对象标识的正文变化后内容哈希更新。确认测试失败（红）
- [x] 3.2 绿实现：按 D3 在一个事务里读出文档章节并写入删除、哈希更新和插入。删除顺序为边、chunk、事件、对象。不新增迁移，打开已有库后 `user_version` 仍为 2。验证 `cargo test -p mnemex-store` 通过

## 4. 索引命令（TDD）

- [x] 4.1 红测试：对临时目录执行 `mnemex index`。删除文件后旧词不再命中；改标题后旧对象不在、新对象在；改正文后旧词不命中、新 chunk 为待索引、内容哈希更新、创建事件仍是一条；文件改成非 UTF-8 后旧章节仍在；文件清空后章节删除；再次索引的目录不存在时原库不变；库里另有一种知识对象时，去掉文本文件不会删掉它；先索引只有 `a.md` 的目录，再把只有 `b.md` 的目录写入同一个库，则 `a.md` 的章节不在、`b.md` 的在；内容不变再索引，对象数与 chunk 数不变；库文件的父目录不出现向量数据集子目录。确认测试失败（红）
- [x] 4.2 绿实现：按 D4，扫描成功后在一个事务里列出文档章节、调用对账计划并提交。扫描失败不写入。仍然不调用 `LanceVectorStore::open`。验证 `cargo test -p mnemex-cli` 通过

## 5. 验收

- [x] 5.1 `cargo build --workspace` 退出码 0
- [x] 5.2 `cargo test --workspace` 退出码 0
- [x] 5.3 `cargo clippy --workspace` 无 error
- [x] 5.4 `cargo tree --workspace` 的直接第三方仍仅为 rusqlite、lancedb、sha2 三系
- [x] 5.5 `openspec validate reconcile-text-ingest --strict` 通过
- [x] 5.6 把 `text-ingest` 与 `sqlite-schema` 这两份 delta 的全部场景逐条指到测试，核对表写在本文件末尾

## Workflow follow-up

- 人工审阅通过后再执行 `/opsx-apply`。
- 验收通过后用 `openspec archive` 归档，不手改 `openspec/specs/`。

## 场景覆盖

| 场景 | 覆盖 |
| --- | --- |
| text-ingest「正文落库且待索引」 | `crates/mnemex-cli/tests/index.rs` `index_writes_searchable_pending_chunks_once` |
| text-ingest「创建事件来自规则」 | 同文件 `index_writes_searchable_pending_chunks_once` |
| text-ingest「不写入向量索引」 | 同文件 `index_writes_searchable_pending_chunks_once` |
| text-ingest「对账之后仍不写入向量索引」 | 同文件 `deleted_file_is_forgotten` |
| text-ingest「文件消失后旧章节删除」 | 同文件 `deleted_file_is_forgotten` |
| text-ingest「标题变化后旧对象删除」 | 同文件 `retitled_section_replaces_the_object` |
| text-ingest「其他类型保留」 | 同文件 `other_kind_survives_when_the_text_file_is_removed` |
| text-ingest「换一个目录索引」 | 同文件 `indexing_another_directory_drops_the_previous_documents` |
| text-ingest「旧正文不再命中」 | 同文件 `rewritten_body_updates_the_hash_and_keeps_one_created_event` |
| text-ingest「内容哈希更新且创建事件保留」 | 同文件 `rewritten_body_updates_the_hash_and_keeps_one_created_event` |
| text-ingest「编码变坏时不删除」 | 同文件 `undecodable_file_keeps_the_indexed_section` |
| text-ingest「清空文件后章节删除」 | 同文件 `emptied_file_drops_the_section` |
| text-ingest「失败的再次扫描保持原库」 | 同文件 `missing_directory_leaves_the_database` |
| sqlite-schema「追加只增不更」 | `crates/mnemex-store/tests/schema.rs` `events_append_only_and_usage_replays` |
| sqlite-schema「回放推导写回」 | 同文件 `events_append_only_and_usage_replays` |
| sqlite-schema「外键拒绝孤儿事件」 | 同文件 `events_append_only_and_usage_replays` |
| sqlite-schema「移除文档章节时带走事件」 | `crates/mnemex-store/tests/reconcile.rs` `removing_a_document_section_drops_its_rows_and_keeps_other_kinds` |
| sqlite-schema「删除后旧词不再命中」 | 同文件 `deleting_a_chunk_drops_it_from_full_text_search` |
| sqlite-schema「章节与关联行一起消失」 | 同文件 `removing_a_document_section_drops_its_rows_and_keeps_other_kinds` |
| sqlite-schema「其他类型及其 chunk 保留」 | 同文件 `removing_a_document_section_drops_its_rows_and_keeps_other_kinds` |

