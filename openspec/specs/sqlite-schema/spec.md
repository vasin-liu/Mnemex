# sqlite-schema Specification

## Purpose
为 Mnemex 的 SQLite 模式层定约：迁移的幂等与版本防线、五类型经稀疏列的无损往返、事件 append-only 与回放推导、chunks 对齐行与 edge 行的外键约束——SQLite（source of truth，ADR-002）之上一切能力的挂载面。

## Requirements

### Requirement: 模式迁移

`mnemex-store` SHALL 以 `PRAGMA user_version` 为版本号维护有序迁移列表：打开时按序应用未执行的迁移，每条迁移在事务内执行并步进版本；已应用版本 SHALL 不重复执行（幂等）。库版本**超前**于程序已知列表时 SHALL 拒绝打开——未知模式不猜测、不静默降级。

#### Scenario: 全新建库应用 v1

- **WHEN** 用尚不存在的路径初始化
- **THEN** 四张表（knowledge_objects / knowledge_events / chunks / knowledge_edge）全部建立，`user_version` 达到当前迁移列表长度

#### Scenario: 重开不重复应用

- **WHEN** 对已迁移到当前版本的库再次初始化
- **THEN** 初始化成功，版本不变，既有数据完好

#### Scenario: 超前版本拒绝打开

- **WHEN** 库的 `user_version` 超过程序已知列表
- **THEN** 初始化返回可辨识的迁移错误，不产出库句柄

### Requirement: KO 行持久化

五种知识对象类型 SHALL 以单表稀疏列布局（公共列 + 各类型特有列，NULL 表示缺省）无损往返：写入后按 id 读取，SHALL 还原为与写入时相等的具体类型（含生命周期字段的 Some/None 组合与引用信息）。枚举值（类型判别、符号种类、freshness）SHALL 以 INTEGER 常量映射落库，未知值读回时 SHALL 报错而非静默猜测。

#### Scenario: 五类型往返无损

- **WHEN** 五种类型各写入一条并按 id 读回
- **THEN** 每条读回值与写入值相等（判别、生命周期、引用、类型特有字段全部一致）

#### Scenario: 缺省语义保持

- **WHEN** 写入带缺省字段（如无模型抽取、无校准、无矛盾分）的对象并读回
- **THEN** 对应字段读回为缺省形态，不出现伪值

#### Scenario: 未知判别值报错

- **WHEN** 行内类型判别列出现程序不认识的 INTEGER
- **THEN** 读取返回错误，不构造出错误类型的对象

### Requirement: 事件追加与回放

事件 SHALL 只追加：事件表以自增序号唯一化，存储层 SHALL NOT 提供事件的更新或删除入口。使用类事件（Used）SHALL 可回放：从事件流推导 `usage_count` 与 `last_used` 并写回对象行。事件行 SHALL 以外键指向所属 KO，不存在的 KO 上追加事件 SHALL 被拒绝。

#### Scenario: 追加只增不更

- **WHEN** 同一 KO 连续追加多条事件
- **THEN** 每条获得自增序号，全部保留，无任何入口改写既有事件行

#### Scenario: 回放推导写回

- **WHEN** 追加 N 条 Used 事件（时间递增）后执行回放同步
- **THEN** 对象行的 `usage_count` 变为 N、`last_used` 变为最新事件时间戳

#### Scenario: 外键拒绝孤儿事件

- **WHEN** 对不存在的 KO 追加事件
- **THEN** 写入被拒绝并返回错误

### Requirement: chunks 对齐行与 edge 行

chunks 行 SHALL 携带 ADR-002 对齐键（`chunk_id` + `content_hash`）与待索引位（默认待索引，可翻转——为 spike 的补偿流预留状态载体）；edge 行 SHALL 携带源、目标与类型。两表的外键 SHALL 指向所属 KO，缺 KO 的写入 SHALL 被拒绝。

#### Scenario: chunk 默认待索引且可翻转

- **WHEN** 插入 chunk 行后读取，再翻转其待索引位
- **THEN** 初始为待索引，翻转后读回新状态

#### Scenario: 缺 KO 的行被拒绝

- **WHEN** 对不存在的 KO 插入 chunk 行或 edge 行
- **THEN** 写入被拒绝并返回错误

### Requirement: chunk 正文

chunk 行 SHALL 保存正文，并 SHALL 能按 chunk 标识读回与写入时相同的正文。正文为空的行 SHALL 仍然携带对齐键，且默认待索引。

#### Scenario: 正文往返

- **WHEN** 插入一条带非空正文的 chunk，再按 chunk 标识读取
- **THEN** 读回的正文与写入时相同

#### Scenario: 空正文仍是对齐行

- **WHEN** 插入一条正文为空的 chunk
- **THEN** 该行存在，对齐键完整，且为待索引

### Requirement: 正文全文检索

写入非空正文后，SHALL 能用正文中的词通过 FTS5 命中该 chunk。未出现在任何正文中的词 SHALL NOT 命中。

#### Scenario: 词命中对应 chunk

- **WHEN** 插入一条正文含有某个词的 chunk，并用该词做全文检索
- **THEN** 命中这一条 chunk

#### Scenario: 无关词不命中

- **WHEN** 用一个所有正文都不包含的词做全文检索
- **THEN** 没有命中

### Requirement: 已有库补上正文列

打开一个已迁移、但尚无正文列的库时，SHALL 在事务内补上正文列与全文索引，并前进版本。原有 chunk 行 SHALL 保留，其正文读回为空。已处于当前版本的库再次打开 SHALL NOT 重复应用该迁移。

#### Scenario: v1 库升级

- **WHEN** 打开一个尚无正文列的已有库
- **THEN** 打开成功，原有 chunk 行仍在，正文为空，版本前进

#### Scenario: 升级后重开

- **WHEN** 对已经补上正文列的库再次打开
- **THEN** 打开成功，版本不变，已写入的正文仍在
