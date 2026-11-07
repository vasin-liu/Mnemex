# Tasks

## 1. 依赖准入（setup，本组无测试欠账）

- [x] 1.1 在 `crates/mnemex-store/Cargo.toml` 加入 `rusqlite = { version = "0.41.1", default-features = false, features = ["bundled"] }`，验证 `cargo build --workspace` 退出码 0（记录首次 bundled 编译耗时：126.7s）
- [x] 1.2 `cargo tree --workspace` 核对第三方面仅 rusqlite 一系，记录 libsqlite3-sys 版本（0.39.1，连同 rusqlite 闭包内 bitflags/fallible-iterator/fallible-streaming-iterator/smallvec 与构建期 cc/pkg-config/vcpkg）；`cargo tree -p mnemex-core` 复核 core 仍零依赖

## 2. StoreError（TDD，单元层）

- [x] 2.1 红测试：`StoreError` 两条变体的 `Display` 文案、`source()` 指向 `rusqlite::Error`、`From<rusqlite::Error>` 可经 `?` 传播；确认测试失败（红）
- [x] 2.2 绿实现：手写 `StoreError` enum 实现 `std::error::Error`（D3 形态），验证 `cargo test -p mnemex-store` 通过

## 3. TempDir 测试助手 + SqliteStore::open（TDD，集成层）

- [x] 3.1 红测试：新建 `crates/mnemex-store/tests/sqlite_infra.rs`——手写 TempDir helper（D4）；用例：新路径建库可执行 SQL、同一库幂等重开且数据完好、目录缺失的非法路径返回 `StoreError` 而非 panic；确认测试失败（红）
- [x] 3.2 绿实现：`sqlite` 模块 + `SqliteStore::open`（打开 → WAL → busy_timeout=5000 → FTS5 验证，D5 顺序），验证 `cargo test -p mnemex-core -p mnemex-store` 通过

## 4. FTS5 可用性保障（TDD，集成层）

- [x] 4.1 红测试：`PRAGMA compile_options` 含 `ENABLE_FTS5`；`temp.` schema 建 fts5 虚拟表插入+查询冒烟通过；`StoreError::Fts5Unavailable` 可构造且错误类别可辨识（单测已在 2.1 组覆盖；本组红来自 `verify_fts5` 未定义的编译失败 + R3 pragmas 集成用例补入）
- [x] 4.2 绿实现：`verify_fts5` 接入 `open` 流程（不可用即拒绝启动），验证 `cargo test -p mnemex-store` 通过
- [x] 4.3 模块文档：`sqlite` 模块英文 doc（FTS5 是 BM25 唯一来源引 ADR-001 第 5 点、带病启动不降级语义），验证 `cargo doc -p mnemex-store --no-deps` 退出码 0

## 5. 验收

- [x] 5.1 `cargo build --workspace` 退出码 0
- [x] 5.2 `cargo test --workspace` 退出码 0，且 `mnemex-store` 测试数 > 0（单元 4 + 集成 5；core 18）
- [x] 5.3 `cargo clippy --workspace` 无 error（warn 基线内，实测零诊断）
- [x] 5.4 `cargo tree --workspace` 第三方仅 rusqlite 一系（rusqlite 0.41.1 → libsqlite3-sys 0.39.1 + bitflags/fallible-iterator/fallible-streaming-iterator/smallvec + 构建期 cc/pkg-config/vcpkg）；`cargo tree -i mnemex-core` 无反向边
- [x] 5.5 `openspec validate add-sqlite-infra --strict` 通过
- [x] 5.6 场景覆盖核对：spec 四需求（初始化/FTS5 保障/库级参数/依赖最小面）的每个场景均能指到至少一个对应测试，核对表落在本文件末尾追加的小节

## 场景覆盖核对表（5.6）

实测：`mnemex-store` 单元 4 + 集成 5、`mnemex-core` 18，全部通过；clippy 零诊断；第三方面仅 rusqlite 一系。

| spec 场景 | 对应测试 |
|---|---|
| 新路径建库成功 | `tests/sqlite_infra.rs::new_path_creates_usable_db`（真实 SQLite + 临时目录，建表/插入/计数） |
| 幂等重开既有库 | `tests/sqlite_infra.rs::reopen_is_idempotent_and_preserves_data`（两次 open，数据回读） |
| 打开失败走 Result | `tests/sqlite_infra.rs::invalid_path_fails_with_store_error`（父目录缺失 → 错误对象，无 panic） |
| bundled 构建冒烟通过 | 单测 `sqlite::tests::verify_fts5_passes_on_bundled_build`（compile_options + temp 表冒烟）+ 集成 `fts5_is_usable_end_to_end`（open 含验证 + 真实 fts5 表 MATCH） |
| FTS5 缺失时拒绝启动 | 错误类别单测 `sqlite::tests::fts5_error_category_is_identifiable` + `verify_fts5` 接入 `open` 的两条失败分支（compile 选项缺失 / 冒烟计数不符）。注：bundled 构建无法真实构造"缺 FTS5"环境，缺失路径以错误类别与接线代码覆盖，属诚实记录的测试边界 |
| WAL 生效 | `tests/sqlite_infra.rs::pragmas_are_in_effect`（journal_mode == wal；open 内非 WAL 硬失败分支亦有覆盖） |
| busy_timeout 生效 | `tests/sqlite_infra.rs::pragmas_are_in_effect`（busy_timeout == 5000） |
| 依赖树检查 | 任务 5.4 命令实测输出（第三方仅 rusqlite 一系，libsqlite3-sys 0.39.1） |

## Workflow follow-up

- 人工审阅四份工件后执行 `/opsx-apply`。
- apply 完成、验收全绿后归档：`openspec archive add-sqlite-infra --yes`（delta 已含 Purpose，预期无需补占位）。
