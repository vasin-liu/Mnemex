# vector-store Specification

## Purpose
为 Mnemex 的向量索引定约：它是 SQLite 的派生存储，按 chunk 对齐键写入和删除，能在丢弃后重建，并且只在向量追平之后清除待索引位。

## Requirements

### Requirement: 对齐键写入与删除

向量索引 SHALL 以 chunk 标识与内容哈希作为对齐键写入调用方提供的向量，并 SHALL 能按该键精确删除。删除一个键 SHALL NOT 移除其他键。本能力不计算嵌入。

#### Scenario: 写入后可按键找到

- **WHEN** 写入一条带向量的对齐键
- **THEN** 按同一 chunk 标识与内容哈希能读回该向量

#### Scenario: 精确删除不影响其他键

- **WHEN** 索引中有两条不同对齐键，并删除其中一条
- **THEN** 被删的键不存在，另一条仍在

### Requirement: 从 SQLite 全量重建

向量索引 SHALL 能被丢弃，并仅根据 SQLite 当前 chunk 行与调用方为这些行提供的向量重建。重建完成后，索引中的对齐键集合 SHALL 与这些 chunk 行一致。索引 SHALL NOT 在缺少调用方向量时自行生成向量。

#### Scenario: 空索引被 SQLite 行补齐

- **WHEN** 向量索引为空、SQLite 中有 chunk 行，并以调用方向量执行重建
- **THEN** 每条 chunk 行的对齐键都出现在索引中

#### Scenario: 重建丢弃已不在 SQLite 的键

- **WHEN** 索引里有一条 SQLite 已不存在的对齐键，并执行重建
- **THEN** 该键不再存在于索引中

### Requirement: 补偿顺序

写入路径 SHALL 先提交 SQLite 的 chunk 行并保持待索引，再写入向量索引。仅当向量写入成功后，该行的待索引位 SHALL 被清除。向量写入失败时，待索引位 SHALL 保持，且索引中不得把该行标成已追平。随后的重建 SHALL 能把仍待索引的行补进索引，并在成功后清位。

#### Scenario: 成功路径最后才清位

- **WHEN** SQLite 已提交一条待索引的 chunk，且对应向量写入成功
- **THEN** 待索引位被清除，且索引中存在该对齐键

#### Scenario: 向量失败保持待索引

- **WHEN** SQLite 已提交一条待索引的 chunk，且向量写入失败
- **THEN** 待索引位仍为待索引，且索引中不存在该对齐键

#### Scenario: 重建补上未追平的行

- **WHEN** 存在待索引的 chunk 行，并以调用方提供的向量执行重建
- **THEN** 该对齐键出现在索引中，且待索引位被清除
