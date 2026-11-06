# Mnemex / 藏识 — 详细设计阶段输出优先级结论

> 基于《Mnemex_v2_架构设计评审结论》建议的 10 项下一阶段输出，给出优先级判断与依赖顺序，作为详细设计阶段的执行计划。

---

## 一、排序依据

判断优先级的核心原则不是"重要程度"，而是**依赖关系**——哪些产出是其他产出的输入，就必须排在前面；同一批次内没有硬依赖关系的产出可以并行推进。具体判断：

- ADR 记录的是 v2 阶段已经拍板的决策（定位收敛、Knowledge Object 分层、MCP 降级为 Adapter 等），只是把口头结论落成书面记录，不依赖任何后续设计，且能防止详细设计阶段重新争论已解决的问题，所以排最前。
- DDD 领域模型是概念层，Knowledge Object schema 是它的具体落地，两者是同一件事的两个抽象层级，应该放在同一批次连续完成，但领域模型先行。
- Memory Engine 的模块拆分（ObjectStore/Retrieval/Ranking/Graph/Lifecycle/ContextBuilder）依赖 Knowledge Object schema 先确定——不知道知识实体长什么样，无法设计围绕它的存储和检索接口。
- Retrieval / Ranking / Context Builder 三者是 Memory Engine 内部模块的详细展开，必须等模块边界定完才能细化，且三者之间也有顺序：检索先于排序，排序结果先于上下文打包。
- Rust Workspace 结构本质是工程scaffolding，一旦模块边界（批次三）确定就可以搭建，可以和批次三后半段/批次四并行，不需要等所有详细设计文档写完。
- API/MCP 设计依赖 Context Builder 的输出格式——Adapter 层暴露什么，取决于核心引擎最终吐出什么样的上下文包。
- Plugin SDK 排最后，因为它的稳定性反过来依赖核心模型（尤其是 Knowledge Object schema）已经稳定，过早定义 SDK 接口会随着核心调整被迫频繁破坏兼容性。

---

## 二、优先级顺序（四个批次）

### 批次一：决策先行
1. **ADR（架构决策记录）**——把 v2 中"定位收敛""Knowledge Object 分层""图谱延迟引入图库""MCP 降级为 Adapter""AI 分级策略"等结论逐条转成正式 ADR 条目，附决策背景、备选方案、取舍理由。

### 批次二：领域与数据模型
2. **DDD 领域模型**——划定限界上下文（Ingestion / Knowledge / Retrieval / Memory / Agent Adapter），定义核心实体与聚合根。
3. **Knowledge Object 数据模型**——`CodeSymbol`/`DesignDecision`/`BugRecord`/`DocumentSection`/`CommitEvent` 的具体字段设计，以及 Memory 生命周期字段（confidence/usage_count/last_used/freshness/contradiction_score）如何挂载在通用结构上。Rust 落地建议用 `trait KnowledgeObject` 而非类继承。

### 批次三：Memory Engine 详细设计
4. **Memory Engine 模块拆分**——ObjectStore、Retrieval、Ranking、Graph、Lifecycle、ContextBuilder 各自的职责边界与对外接口（trait 定义）。
5. **Retrieval Pipeline**——Fast Path（Keyword+Vector）与 Deep Path（+Graph Expansion）的具体触发条件、路由逻辑。
6. **Ranking Pipeline**——Reranker 选型（本地小模型 vs 规则打分）、与 Memory 生命周期字段（freshness/confidence）如何参与排序权重。
7. **Context Builder**——最终提供给 Agent 的上下文包格式，需要兼顾 token 预算控制和引用来源可追溯。

> 5-7 三者可以在完成第 4 项模块接口定义后并行展开细化，但设计评审顺序建议还是按 检索→排序→打包 走查一遍，确保接口衔接一致。

### 批次四：工程落地与对外接口
8. **Rust Workspace 结构**——crate 划分（如 `mnemex-core`/`mnemex-ingest`/`mnemex-retrieval`/`mnemex-adapter-mcp` 等）与依赖方向，可在批次三模块边界确定后即启动，与 5-7 项并行推进。
9. **API / MCP 设计**——Adapter 层的具体接口契约，依赖 Context Builder 的输出格式先确定。
10. **Plugin SDK**——待核心模型稳定后最后产出，避免过早锁定接口导致后续核心调整时被迫破坏兼容性。

---

## 三、并行关系说明

- 批次内部分产出可并行（如批次二的 DDD 模型与 schema 设计可以边写边对齐；批次四的 Workspace 搭建可以先于 API/MCP 设计启动）。
- 批次之间存在硬依赖，不建议跳批次并行——尤其是在 Knowledge Object schema（批次二）稳定前启动 Memory Engine 详细设计（批次三），大概率会导致返工。

---

## 四、建议

按此顺序推进，预计第一个可以独立评审通过的里程碑是**批次二结束**（领域模型 + Knowledge Object schema），这是后续所有详细设计的地基，值得在进入批次三之前单独做一轮评审确认，而不是等到批次四结束才一次性回顾。
