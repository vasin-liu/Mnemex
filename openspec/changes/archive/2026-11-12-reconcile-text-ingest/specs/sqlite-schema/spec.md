# Spec Delta

## MODIFIED Requirements

### Requirement: 事件追加与回放

事件 SHALL 只追加：事件表以自增序号唯一化。对象仍在库中时，存储层 SHALL NOT 提供事件的更新或删除入口。对账移除文档章节时，SHALL 在同一事务内删除且仅删除该对象的事件行。使用类事件（Used）SHALL 可回放：从事件流推导 `usage_count` 与 `last_used` 并写回对象行。事件行 SHALL 以外键指向所属 KO，不存在的 KO 上追加事件 SHALL 被拒绝。

#### Scenario: 追加只增不更

- **WHEN** 同一 KO 连续追加多条事件
- **THEN** 每条获得自增序号，全部保留，无任何入口改写既有事件行

#### Scenario: 回放推导写回

- **WHEN** 追加 N 条 Used 事件（时间递增）后执行回放同步
- **THEN** 对象行的 `usage_count` 变为 N、`last_used` 变为最新事件时间戳

#### Scenario: 外键拒绝孤儿事件

- **WHEN** 对不存在的 KO 追加事件
- **THEN** 写入被拒绝并返回错误

#### Scenario: 移除文档章节时带走事件

- **WHEN** 对账删除一个仍有创建事件的文档章节
- **THEN** 该对象及其事件行都不存在，其他对象的事件仍在

## ADDED Requirements

### Requirement: 删除 chunk 后全文索引同步

删除一条 chunk 后，该行 SHALL 不存在，全文检索 SHALL NOT 再命中其正文。其他 chunk SHALL 不受影响。

#### Scenario: 删除后旧词不再命中

- **WHEN** 两条 chunk 的正文不同，删除其中一条，并用被删正文中不少于 3 个字符的词做全文检索
- **THEN** 没有命中，另一条仍可按其正文中的词命中

### Requirement: 移除文档章节

移除一个文档章节 SHALL 在同一事务内删除指向它的边、它的 chunk 和它的事件行，然后删除该对象。其他知识对象及其 chunk SHALL 保留。

#### Scenario: 章节与关联行一起消失

- **WHEN** 一个文档章节有 chunk、创建事件，且有一条边指向它，然后移除该章节
- **THEN** 该对象、其 chunk、其事件和那条边都不存在

#### Scenario: 其他类型及其 chunk 保留

- **WHEN** 库中同时有一个文档章节和一个其他类型的知识对象，并移除该文档章节
- **THEN** 其他类型的对象及其 chunk 仍在
