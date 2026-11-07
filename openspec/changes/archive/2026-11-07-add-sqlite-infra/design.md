# Design

## Context

`mnemex-store` 只有空 `VectorStore` trait；SQLite 是 source of truth（ADR-002），FTS5 是 BM25 唯一来源（ADR-001 第 5 点）。本 change 只做最小基础设施（切法二），表结构与补偿流留给下一个 store change。约束：依赖准入四要素 + 上游健康度须记录在案；错误类型依赖解锁前用手写 std 路径；生产路径无 `unwrap`/`expect`；测试不 mock 文件系统。

## Goals / Non-Goals

**Goals:**

- 依赖准入流程首次完整跑通（模板供后续 tree-sitter/notify 照抄）
- `SqliteStore::open` 幂等建库 + WAL/busy_timeout + FTS5 验证，全部有集成测试
- 把"bundled 是否含 FTS5"从假设变成测试结论

**Non-Goals:**

- 四张表结构与迁移、事件/chunk/edge 持久化（下一个 store change）
- `VectorStore` trait 方法签名、补偿流（ADR-002 落地 change）
- 备份/恢复（ADR-001 退出标准之一，属 spike 范围）

## Decisions

### D1 依赖：rusqlite 0.41.1，`default-features = false` + `bundled`

| 要素 | 记录 |
|---|---|
| 名称/版本 | `rusqlite` 0.41.1（crates.io，2026-10-28 发布） |
| 许可证 | MIT（默认层合规；SQLite 本体公有领域，bundled 随包编译） |
| MSRV | 1.89 ≤ workspace 1.94 |
| 上游健康度 | ~4 个月一发节奏；无影响 0.41.1 的 RUSTSEC（RUSTSEC-2021-0128 修复于 0.26.2；RUSTSEC-2022-0090 属 libsqlite3-sys，修复于 0.25.1） |
| 特性 | 仅 `bundled`；关掉 default（cache/ffi-sqlite-wasm-rs 不需要，最小面） |
| 引入位置 | 仅 `mnemex-store`（依赖方向红线：store 只多这一项第三方） |

备选：`sqlx`（async 生态超出本阶段需要）、系统 SQLite 绑定（Windows 分发负担）、`tempfile`/`thiserror`（都拒绝——见 D3/D4）。

### D2 FTA 处置：bundled 含 FTS5 由测试裁决

"libsqlite3-sys bundled 编译是否启用 FTS5"不在本设计中断言。`open` 内验证两步：`PRAGMA compile_options` 含 `ENABLE_FTS5`，且在**临时 schema**（`temp.` 前缀）建 fts5 虚拟表做一次插入+查询冒烟（不写用户库）。集成测试即结论：若 bundled 实际不含 FTS5，测试红，本 change 内解决（调整 bundled 编译参数或换绑定）并记录，不留假设出库。

### D3 `StoreError` 手写 enum

```rust
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Fts5Unavailable { detail: String },
}
```
实现 `std::error::Error`（`source()` 指向 `rusqlite::Error`）+ `Display`；`From<rusqlite::Error>` 供 `?` 传播。不引 thiserror——依赖最小面优先，样板量在两条 enum 变体可承受。备选 thiserror（又一张第三方票，本阶段不买）。

### D4 集成测试临时目录：手写 helper，不引 tempfile

`tests/` 内一个 `TempDir` 小助手：`std::env::temp_dir()` + 进程号 + 纳秒时钟 + 原子计数器拼唯一路径，`Drop` 递归删除（删除前先 drop 连接，规避 Windows 句柄滞留）。AGENTS 集成层要求"真实 SQLite 与临时目录"，tempfile 是便利不是必需——为它破依赖最小面不值。备选 tempfile（MIT，健康；若后续表结构 change 测试量大再评估，仍须独立 change）。

### D5 API 形态：`SqliteStore` 封装连接

```rust
pub struct SqliteStore {
    conn: Connection,          // 私有，不外漏裸连接
}
impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, StoreError>;
}
```
`open` 顺序：打开/建库 → `journal_mode=WAL` → `busy_timeout=5000` → FTS5 验证（D2）。WAL 理由：增量索引写与 UI/检索读并存的桌面单机场景；busy_timeout 对应 ADR-001 退出标准里的 Windows 文件锁场景，5000ms 是桌面可感知上限内的保守值（实现可调常量）。备选 `synchronous=FULL`（WAL 下默认 NORMAL 即够，单机无跨机复制）。表结构 change 将在此结构上加迁移与表访问方法，不重开设计。

### D6 测试分层

- 单元（`#[cfg(test)]`）：`StoreError` 的 Display/`source` 链（无 IO，毫秒级）。
- 集成（`tests/sqlite_infra.rs`）：真实 SQLite + 临时目录——新建库执行 SQL、幂等重开、非法路径错误传播、WAL/busy_timeout 生效、FTS5 冒烟。不 mock 文件系统。

## Risks / Trade-offs

- [bundled C 编译：构建期需 MSVC C 编译器] → 本机已具备 Rust/MSVC 工具链；首次构建变慢是记录项，验收时记录耗时，不阻塞。
- [bundled 若不含 FTS5，D2 测试转红] → 在本 change 内解决（改编译参数/换绑定），change 不带假设出库。
- [手写 TempDir 清理在 Windows 上可能残留] → 删除前 drop 连接；残留仅影响测试目录卫生，不影响行为断言；CI 容忍（当前无 CI，风险记账）。
- [5000ms busy_timeout 是经验值] → 常量可调；设备矩阵实测（Phase 0 末）后再议。

## Migration Plan

纯新增：`mnemex-store` 加依赖与 `sqlite` 模块，`Cargo.lock` 首次出现第三方条目。无既有行为可破坏；回滚 = revert 本 change 的 commit（`Cargo.lock` 随之还原）。

## Open Questions

（无——FTA 已由 D2 的测试裁决方案覆盖；表结构问题留给下一个 change。）
