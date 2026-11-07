# Proposal

## Why

批次一 ADR 与 workspace 脚手架已归档，`mnemex-core` 的 ko/lifecycle/event/ids 四个模块仍只有文档注释。按《详细设计阶段输出优先级结论》批次序，Knowledge Object 数据模型是下一项硬交付（批次二）：store 的表结构、ingest 的产物形状、retrieval 的返回类型、MCP 暴露的差异字段全部引用它——它不定稿，后续所有 change 只能在 ADR 文字上悬空。且该 change 零第三方依赖，是全项目第一个完整红→绿 TDD 循环的最低成本起点。

## What Changes

- `mnemex-core::ids`：`KoId` / `ChunkId` / `ContentHash` / `CommitSha` 不透明 newtype，`SourceRefs`（KO id + content_hash + commit 为主、路径为辅，ADR-011/ADR-002 引用语义）。
- `mnemex-core::lifecycle`：`Lifecycle` 结构——`model_confidence`（原始）与 `confidence`（校准后）拆分（ADR-010）、`usage_count`、`last_used`、`freshness`（确定性规则来源，不走模型打分，ADR-007）、`contradiction_score`。
- `mnemex-core::ko`：五种 KO 类型（`CodeSymbol` / `DesignDecision` / `BugRecord` / `DocumentSection` / `CommitEvent`）的具体字段 + `trait KnowledgeObject`（组合而非继承，批次二落地建议）+ `Kind` 判别。
- `mnemex-core::event`：`KnowledgeEvent` 枚举与事件溯源不变式——操作（验证 / 用户标记过时 / 标记矛盾 / 归档 / 反馈 / 使用）一律是追加事件，不直接改数值字段；事件带来源标记（规则 / 模型 / 用户，原型设计 v1 §"操作区"与时间线语义）。
- 单元测试：newtype 防混用、事件不可变、字段不变式。

**不改动**既有 crate 依赖方向；**不引入**任何第三方依赖（含 serde、时间库、哈希算法库——`ContentHash` 为不透明字节，算法由 ingest change 届时走依赖准入）。

## Capabilities

### New Capabilities

- `knowledge-object-model`: `mnemex-core` 知识对象领域模型的类型契约——五类型字段集、生命周期字段语义（含 model_confidence/confidence 拆分）、事件溯源不变式、id 与引用语义。它是 store / ingest / retrieval / adapter 各能力的共享词汇层。

### Modified Capabilities

（无——`rust-workspace` 的零依赖与依赖方向要求已覆盖本 change 的工程约束，无需求变化。）

## Impact

- 代码：仅 `crates/mnemex-core/src/` 四个模块由注释骨架落为具体类型与单元测试；其他 crate 不动（它们尚未引用这些类型）。
- 依赖：零新增第三方依赖。
- 后续接口：store 表结构、ingest 产物、MCP 差异字段（confidence/freshness/contradiction_score）以此为唯一词汇来源。
- 出界（留后续 change）：SQLite 表结构与迁移（store change）；哈希算法与生成策略（ingest change）；序列化格式（MCP/HTTP adapter change）；校准逻辑（路线图外后置项）；L1.5 抽取（Phase 2）。
