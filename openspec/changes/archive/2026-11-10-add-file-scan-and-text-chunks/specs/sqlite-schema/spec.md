# Spec Delta

## ADDED Requirements

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
