# Tasks

## 1. 依赖准入

- [x] 1.1 在 `mnemex-ingest` 加入 `sha2` 0.11.1（MIT OR Apache-2.0），`Cargo.toml` 注释写明版本与许可证。验证：版本钉死为 0.11.1，`cargo tree -p mnemex-ingest --depth 1` 的直接第三方只有 `sha2`

## 2. 模式 v2（TDD）

- [x] 2.1 红测试：带正文的 chunk 能按 id 读回；空正文仍是待索引的对齐行；不少于 3 个字符的词能经 FTS5 命中，无关词不命中；把只含 v1 DDL 且 `user_version = 1` 的库打开后正文列存在、旧行正文为空、版本为 2，再次打开版本不变且后写入的正文还在。确认测试失败（红）
- [x] 2.2 绿实现：v2 迁移（`body` 列、`chunks_fts` trigram 外部内容表、同步触发器、rebuild）、`insert_chunk` 增加正文参数，并改掉 `schema.rs`、`vector.rs` 与 `tests/vector.rs` 里的旧调用（D5）。验证 `cargo test -p mnemex-store` 通过

## 3. 扫描与分块（TDD）

- [x] 3.1 红测试：临时目录里 `.md` / `.txt` 被收录、`.rs` 被跳过；`.git`、`target`、`node_modules` 之下的文件被跳过；非 UTF-8 不使扫描失败；ATX 标题产生 `/` 标题路径，围栏代码里的 `#` 不算标题；超过 512 个标量值的正文被拆开且拼接还原；空文件没有 chunk；不同正文的哈希都是 32 字节且互不相同。确认测试失败（红）
- [x] 3.2 绿实现：按 D3、D4、D7 完成扫描、分块、SHA-256 与 id 派生。`mnemex-ingest` 不依赖 `mnemex-store`。验证 `cargo test -p mnemex-ingest` 通过

## 4. 索引命令（TDD）

- [x] 4.1 红测试：通过 `mnemex index --dir <目录> --db <库>` 索引一份短 Markdown。缺少 `--dir` 时失败且不创建库文件。成功路径上全文检索命中正文、chunk 为待索引、新对象有一条规则来源的创建事件；库文件的父目录不出现向量数据集子目录。同一目录再索引一次，对象数与 chunk 数不变。确认测试失败（红）
- [x] 4.2 绿实现：CLI 依赖 `mnemex-store`，用 std 解析参数，只打开 SQLite 并写入尚不存在的对象与 chunk（D2、D6）。验证 `cargo test -p mnemex-cli` 通过

## 5. 验收

- [x] 5.1 `cargo build --workspace` 退出码 0
- [x] 5.2 `cargo test --workspace` 退出码 0
- [x] 5.3 `cargo clippy --workspace` 无 error
- [x] 5.4 `cargo tree --workspace` 的直接第三方仅为 rusqlite、lancedb、sha2 三系；`cargo tree -p mnemex-core` 不依赖其他 `mnemex-*` crate
- [x] 5.5 `openspec validate add-file-scan-and-text-chunks --strict` 通过
- [x] 5.6 把下面三份规格的全部场景逐条指到测试或命令，核对表写在本文件末尾：`text-ingest`、`sqlite-schema` 新增三条、`sqlite-infra`「依赖树检查」

## 场景覆盖

| 场景 | 覆盖 |
| --- | --- |
| text-ingest「收录 Markdown 与纯文本」 | `crates/mnemex-ingest/tests/scan.rs` `scans_text_extensions_and_skips_the_rest` |
| text-ingest「跳过拒绝名单」 | 同文件 `skips_denied_directories` |
| text-ingest「非 UTF-8 不中断扫描」 | 同文件 `invalid_utf8_does_not_fail_the_scan` |
| text-ingest「标题切成章节」 | 同文件 `atx_headings_build_a_slash_path_and_fences_hide_hashes` |
| text-ingest「超长正文再拆」 | 同文件 `long_body_splits_on_the_scalar_cap_and_on_newlines` |
| text-ingest「空文件无 chunk」 | 同文件 `empty_file_produces_no_chunk` |
| text-ingest「不同正文哈希不同」 | 同文件 `chunk_hashes_are_sha256_and_differ_when_bodies_differ` |
| text-ingest「相同内容再次扫描」 | `crates/mnemex-cli/tests/index.rs` `index_writes_searchable_pending_chunks_once` |
| text-ingest「正文落库且待索引」 | 同文件 `index_writes_searchable_pending_chunks_once` |
| text-ingest「创建事件来自规则」 | 同文件 `index_writes_searchable_pending_chunks_once` |
| text-ingest「不写入向量索引」 | 同文件 `index_writes_searchable_pending_chunks_once`（库目录无子目录，chunk 仍待索引） |
| text-ingest「缺少目录」 | 同文件 `missing_dir_fails_without_creating_a_database` |
| sqlite-schema「正文往返」 | `crates/mnemex-store/tests/schema.rs` `chunk_body_roundtrips_and_empty_body_stays_pending` |
| sqlite-schema「空正文仍是对齐行」 | 同文件 `chunk_body_roundtrips_and_empty_body_stays_pending` |
| sqlite-schema「词命中对应 chunk」 | 同文件 `fts_hits_a_word_in_the_body_and_misses_an_absent_word` |
| sqlite-schema「无关词不命中」 | 同文件 `fts_hits_a_word_in_the_body_and_misses_an_absent_word` |
| sqlite-schema「v1 库升级」 | `crates/mnemex-store/src/schema.rs` `v1_database_gains_an_empty_body_and_keeps_later_text` |
| sqlite-schema「升级后重开」 | 同测试 `v1_database_gains_an_empty_body_and_keeps_later_text` |
| sqlite-infra「依赖树检查」 | `cargo tree -p mnemex-ingest --depth 1` 的直接第三方只有 `sha2` 0.11.1。`cargo tree -p mnemex-store --depth 1` 的直接第三方是 `rusqlite`，以及 lancedb 系的 `lancedb`、`arrow-array`、`arrow-schema`、`futures`、`tokio`。`digest` 在 sha2 的传递闭包里。`cargo tree -p mnemex-core` 没有依赖 |

## Workflow follow-up

- 人工审阅工件后执行 `/opsx-apply`。
- apply 完成、验收全绿后归档：`openspec archive add-file-scan-and-text-chunks --yes`。
