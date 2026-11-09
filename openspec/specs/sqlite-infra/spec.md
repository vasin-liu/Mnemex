# sqlite-infra Specification

## Purpose
为 Mnemex 的 SQLite 本地库基础设施定约：初始化的幂等性与错误传播、FTS5 可用性保障（Fast Path 的 BM25 唯一来源）、库级运行参数，以及首次引入第三方依赖后的最小依赖面。

## Requirements

### Requirement: SQLite 库初始化

`mnemex-store` SHALL 提供在指定路径创建或打开本地 SQLite 库的入口：路径不存在则创建，已存在则打开且不破坏既有内容；同一库重复打开 SHALL 幂等。路径、IO、SQLite 级错误 SHALL 经 `Result` 逐层传播，不得 panic。

#### Scenario: 新路径建库成功

- **WHEN** 用一个尚不存在的文件路径初始化
- **THEN** 库文件创建成功，且可在其上执行 SQL

#### Scenario: 幂等重开既有库

- **WHEN** 对同一已存在的库再次初始化
- **THEN** 初始化成功，且库内既有数据完好无损

#### Scenario: 打开失败走 Result

- **WHEN** 初始化时传入不可用的路径（目录缺失或非法）
- **THEN** 返回错误对象而非 panic，错误信息可辨识失败原因

### Requirement: FTS5 可用性保障

初始化 SHALL 验证 FTS5 扩展可用（编译选项含 `ENABLE_FTS5`，且能在临时 schema 中创建并使用 fts5 虚拟表做一次冒烟读写）。FTS5 不可用时初始化 SHALL 以可辨识的错误类别拒绝启动，不静默降级——BM25 关键词检索只来自 SQLite FTS5（ADR-001 第 5 点），带病启动会让 Fast Path 在运行期才暴露问题。

#### Scenario: bundled 构建冒烟通过

- **WHEN** 在 bundled 编译（自带 SQLite 源码）的构建上初始化
- **THEN** FTS5 验证通过，初始化成功

#### Scenario: FTS5 缺失时拒绝启动

- **WHEN** FTS5 扩展不可用
- **THEN** 初始化返回 FTS5 专属的错误类别，不产出"可用"的库句柄

### Requirement: 库级运行参数

初始化 SHALL 设置面向桌面单机读写的库级参数：WAL 日志模式（后台增量索引写与前台读并存）、busy_timeout（应对 Windows 文件锁与短暂并发冲突），以及外键约束开启（`foreign_keys`，随模式层外键引入、按连接显式开启）。全部参数 SHALL 在初始化后可查询验证为已生效；初始化 SHALL 在参数设置之后执行模式迁移（见 `sqlite-schema` 能力）。

#### Scenario: WAL 生效

- **WHEN** 初始化完成后查询 `journal_mode`
- **THEN** 值为 `wal`

#### Scenario: busy_timeout 生效

- **WHEN** 初始化完成后查询 `busy_timeout`
- **THEN** 值为初始化时设定的非零毫秒数

#### Scenario: 外键开启生效

- **WHEN** 初始化完成后查询 `foreign_keys`
- **THEN** 值为开启，且对外键违规的写入被数据库拒绝

### Requirement: 第三方依赖最小面

本 change 引入后，workspace 第三方依赖 SHALL 仅包含 `rusqlite` 及其 bundled 传递依赖；测试与实现 SHALL NOT 借机引入其他第三方库（如 tempfile、thiserror）。此后新增任何依赖仍须以独立 change 提出（`rust-workspace` 依赖准入红线）。

#### Scenario: 依赖树检查

- **WHEN** 执行 `cargo tree --workspace` 并过滤出 `mnemex-*` 之外的条目
- **THEN** 仅出现 rusqlite 一系（rusqlite、libsqlite3-sys 及其 bundled 链）
