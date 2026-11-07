# knowledge-object-model Specification

## Purpose
为 Mnemex 的知识对象领域模型定约：五种知识对象类型、生命周期字段语义、事件溯源不变式与引用语义，作为 store / ingest / retrieval / adapter 各能力的共享词汇层。

## Requirements

### Requirement: 知识对象五类型

`mnemex-core` SHALL 提供五种知识对象类型：`CodeSymbol`（函数/类/模块级代码单元，归属调用/继承/引用等结构关系）、`DesignDecision`（问题-诊断-方案三元组）、`BugRecord`（调试记录、根因、修复方案）、`DocumentSection`（文档按章节切分的语义单元）、`CommitEvent`（Git 提交作为一等事件，关联受影响对象）。五种类型 SHALL 通过统一契约暴露对象 id、类型判别、生命周期信息与引用信息；文档与代码属于同一对象空间，不是纯代码图。

#### Scenario: 统一契约访问

- **WHEN** 构造任一种知识对象实例并通过统一契约访问
- **THEN** 能取回其对象 id、类型判别、生命周期信息与引用信息，且五种类型走同一访问路径

#### Scenario: 类型判别完备

- **WHEN** 枚举类型判别
- **THEN** 恰好覆盖五种知识对象类型，不存在第六种

### Requirement: 生命周期字段语义

每个知识对象 SHALL 携带生命周期信息：`model_confidence`（模型原始分，无模型抽取时缺省）、`confidence`（校准后分，校准能力落地前恒为缺省）、`usage_count`（被检索/引用计数）、`last_used`（最近使用时间）、`freshness`（新鲜度，仅由确定性规则来源——git/内容哈希/修改时间——产生，不存在模型打分入口）、`contradiction_score`（矛盾分，仅对近邻候选计算过才有值）。`model_confidence` 与 `confidence` SHALL 是互不混用的独立字段。

#### Scenario: 置信度双字段分离

- **WHEN** 在需要校准分的位置传入模型原始分（或反之）
- **THEN** 类型系统拒绝混用，两种分数各自独立存放

#### Scenario: 校准前缺省

- **WHEN** 校准能力未上线（当前系统状态）
- **THEN** 任何知识对象的 `confidence` 字段取缺省值，原始分只落在 `model_confidence`

#### Scenario: freshness 仅规则来源

- **WHEN** 产生或更新 freshness 值
- **THEN** 只能经确定性规则来源构造，API 上不存在模型评分来源

### Requirement: 事件溯源不变式

对知识对象的一切操作——创建、被检索使用、用户标记已验证、用户标记过时、标记矛盾、归档、有用性反馈——SHALL 表示为不可变事件；操作 SHALL NOT 直接修改数值字段（置信度、计数、新鲜度），状态只能由事件流推导。每个事件 SHALL 记录对象 id、时间戳与来源标记，且来源能区分规则、模型、用户三类（用于展示层区分"规则判定"与"你标记为"）。

#### Scenario: 操作即事件

- **WHEN** 发生任一上述操作
- **THEN** 产生对应的事件变体，而对象的数值生命周期字段不被该操作直接写入

#### Scenario: 事件不可变

- **WHEN** 一个事件构造完成
- **THEN** 不存在任何入口修改其已记录的内容

#### Scenario: 事件来源可辨

- **WHEN** 读取任一事件
- **THEN** 能区分其来源是规则触发、模型抽取还是用户操作

### Requirement: 引用语义

对象引用 SHALL 以"对象 id + content_hash + commit"为主、路径为辅；对象 id、chunk id、content_hash、commit 标识 SHALL 为互不混用的不透明类型；content_hash 不承诺特定哈希算法，算法选择留给摄取阶段。

#### Scenario: 标识防混用

- **WHEN** 在需要对象 id 的位置传入 chunk id、content_hash 或 commit 标识
- **THEN** 编译期拒绝，四种标识互不隐式转换

#### Scenario: 主辅引用结构

- **WHEN** 表达对某知识对象的引用
- **THEN** 引用结构以 id、content_hash、commit 为主键字段、路径为辅助字段，路径缺省或变化不影响主引用成立
