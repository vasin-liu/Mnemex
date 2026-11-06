# decision-records Specification

## Purpose
约束批次一架构决策记录（ADR-001 至 ADR-012，加 GPUI 观察记录）的覆盖面、六段结构与基线忠实性：ADR 只转写 v3/v3.1 已定稿的结论，不引入新决策，不出现评审已否决的表述；引用必须指向基线文档的具体小节。

## Requirements

### Requirement: 批次一 ADR 覆盖面

项目 SHALL 在 `docs/adr/` 下维护批次一 ADR：编号 ADR-001 至 ADR-012，覆盖 v3/v3.1 定稿的十二项决策（向量库、双库一致性、embedding 选型与许可证、摘要运行时与 L2 档位、Kuzu 移除、MCP dual-era、Fast Path RRF、评测集门禁、借力 vs 自研、L1.5 决策层、Agent 工件读写分离、扫描页策略）；GPUI 评估以观察记录单独存档，不占 ADR 编号。

#### Scenario: 文件齐全

- **WHEN** 列出 `docs/adr/` 目录并与 `docs/adr/README.md` 索引对照
- **THEN** 存在 12 份 ADR 文件与 1 份 GPUI 观察记录，索引条目与文件一一对应

#### Scenario: 主题映射

- **WHEN** 按编号检查各 ADR 主题
- **THEN** 与 design.md 第 3 节的编号映射表逐条一致，无缺项

### Requirement: ADR 结构完整性

每份 ADR SHALL 含六个部分：状态（Proposed/Accepted/Superseded）、背景、决策、备选方案、后果、引用；引用 SHALL 指向基线文档的具体小节而非泛指。

#### Scenario: 六段齐全

- **WHEN** 逐份检查 `docs/adr/ADR-*.md`
- **THEN** 六个部分全部存在，且"引用"指向 v3/v3.1 的具体章节号或小节名

#### Scenario: 状态标记

- **WHEN** 检查本批次 ADR 的状态字段
- **THEN** 全部为 Accepted（基线已定稿）；后续被替代时标 Superseded 并指向替代 ADR

### Requirement: ADR 与基线一致

ADR 内容 SHALL 是 v3/v3.1 已定稿结论的忠实转写，不引入新决策、不修改已评审语义；与基线冲突处以基线为准。

#### Scenario: 无新决策

- **WHEN** 审阅各 ADR 的"决策"部分
- **THEN** 每一条都能在引用的基线小节中找到对应表述，无凭空新增的技术决策

#### Scenario: 评审语义保持

- **WHEN** 对照 v3/v3.1 交叉评审回填记录检查易漂移点（如 freshness 确定性、denylist 语义、L1.5 相位、出云无撤销）
- **THEN** ADR 表述与回填后的定稿语义一致，不出现初稿中已被评审否决的表述
