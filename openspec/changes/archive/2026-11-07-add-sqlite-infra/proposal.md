# Proposal

## Why

`mnemex-store` 目前是空 trait，而它是所有后续工作的底座：ADR-001 spike 的第一项退出标准（SQLite↔LanceDB 一致性）需要 SQLite 侧存在；表结构、事件存储、edge 图、Fast Path 的 BM25（唯一来源是 SQLite FTS5，ADR-001 第 5 点）全部压在它上面。同时，本仓库至今零第三方依赖，**首次走依赖准入流程需要一个小而典型的靶子**——准入流程跑通本身就是本 change 的交付物之一，它不该和表结构设计互相污染（探索结论：切法二）。

## What Changes

- **首次依赖准入**：引入 `rusqlite 0.41.1`（MIT，MSRV 1.89 ≤ 1.94），仅进 `mnemex-store`，`default-features = false` + `bundled` 特性（自带 SQLite C 源码编译，Windows 免系统依赖）。上游健康度：2026-10-28 发布、约 4 个月一发节奏、无影响当前版本的 RUSTSEC 公告（最近两条：RUSTSEC-2021-0128 修复于 0.26.2，RUSTSEC-2022-0090 属 libsqlite3-sys 且修复于 0.25.1）。
- **`mnemex-store` 新增 `sqlite` 模块**：`SqliteStore::open`（新路径建库、已存在路径幂等打开）、库级参数（WAL + busy_timeout，对应后台索引写/前台读并存与 Windows 文件锁场景）、`StoreError`（手写 enum 实现 `std::error::Error`，不引 thiserror）。
- **FTS5 可用性保障**：初始化时验证 FTS5（`PRAGMA compile_options` 含 `ENABLE_FTS5` + 临时 fts5 虚拟表冒烟），不可用则拒绝启动并返回可辨识错误，不静默降级——同时把"bundled 是否含 FTS5"这个待验证假设（FTA）变成测试结论。
- **集成测试**：真实 SQLite + 临时目录（手写 tempdir helper，不引 tempfile）。

## Capabilities

### New Capabilities

- `sqlite-infra`: SQLite 本地库基础设施的运行行为——初始化幂等与错误传播、FTS5 可用性保障、库级运行参数、第三方依赖最小面。

### Modified Capabilities

（无——`rust-workspace` 的依赖准入要求不变，本 change 是遵守该要求的首个实例；`VectorStore` trait 与表结构不动。）

## Impact

- 代码：`crates/mnemex-store`（新增 `rusqlite` 依赖与 `sqlite` 模块）；根 `Cargo.lock` 首次出现第三方条目。
- 依赖面：workspace 第三方从零变为 rusqlite 一系（含 bundled 传递依赖 libsqlite3-sys）；构建期需要 C 编译器（MSVC）。
- 出界（留后续 change）：四张表结构与迁移（`knowledge_objects`/`knowledge_events`/`chunks`/`knowledge_edge`）、事件与 chunk 持久化、`VectorStore` trait 方法签名、双库补偿流实现（ADR-002）、LanceDB spike（ADR-001）。
