# Spec Delta

## MODIFIED Requirements

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
