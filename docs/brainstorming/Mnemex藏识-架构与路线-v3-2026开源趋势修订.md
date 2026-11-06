# Mnemex / 藏识 — 架构与路线 v3（2026-11 开源趋势修订 · 三模型交叉评审回填版）

> 本文基于 2026 年 11 月对开源生态的调研，对《Mnemex藏识-架构与路线-v2-评审整合版》做出修订，作为后续实现的当前基线。v2 及更早文档保留作为历史记录，本文档中的结论优先级更高。
>
> **修订流程**：初稿完成后，由三个不同模型的独立评审代理交叉评审（评审方 A / B / C），三方结论均为"有条件通过"（6.5 / 6.5 / 6 分）。本版已回填全部共性 blocker 与主要 major 意见，回填记录见第十节。**总体架构分层与领域模型维持 v2 冻结状态不变，本次修订技术选型、MCP 适配基线与路线图节奏。**
>
> 修订动因是三类外部变化：① MCP 2026-07-28 规范正式定稿；② 部分候选依赖的维护状态发生实质变化（Kuzu 归档、sqlite-vec 停滞）；③ "代码索引 MCP 服务器"赛道在 2026 年快速商品化。

---

## 一、修订范围总览

| 维度 | 判定 | 说明 |
|---|---|---|
| 产品定位（本地工程记忆引擎） | ✅ 保持 | 通用 agent memory 库均不以代码符号+工程决策为一等公民，定位反而更成立 |
| 架构分层（Knowledge Object / Engine-Adapter 解耦 / Fast-Deep 双路径） | ✅ 保持冻结 | 逐项被趋势验证，见第二节 |
| 图谱策略（SQLite-first） | ✅ 保持，删除 Kuzu 候选 | Kuzu 上游已死，从选型表移除 |
| 向量库选型 | ⚠️ **LanceDB 定为首选，sqlite-vec 保留为回退**；Phase 0 带四项退出标准的 Spike，不做无验证定稿 | 回填评审意见：活跃度只能淘汰候选，不能替代嵌入式适配验证 |
| Embedding / 本地摘要模型 | ⚠️ 脱离 CC BY-NC 新系 jina-code-embeddings；Apache-2.0 阵营候选经内部评测与许可证审查后锁定默认档 | 回填：v1 点名的 jina-embeddings-v2-base-code 本身是 Apache-2.0，不作废；EmbeddingGemma-2 有 Gemma 附加使用条款，不列入纯 Apache 阶梯 |
| 路线图节奏 | ⚠️ **"薄索引面 / 厚记忆对象"**，而非笼统的"Phase 1-2 压薄" | 回填：Phase 1 实际新增交付物，薄的是语言数/tool 数/解析精度，厚的是 KO 生命周期 |
| 内部评测集 | ⚠️ 升级为 Phase 1 硬性交付物，**定位为回归门禁**，不承担"击败竞品"的证明责任 | 回填评审意见 |
| MCP 适配 | ⚠️ **dual-era**：2026-07-28 无状态路径为主，保留 `initialize` 兼容旧客户端 | 回填 blocker：主流客户端（Cursor 等）仍走旧握手，modern-only 会被市场隔离 |

---

## 二、被趋势验证、维持不变的决策

### 2.1 Memory Engine / Adapter 解耦

MCP 2026-07-28 规范正式定稿：协议核心转向**无状态**——`initialize` 握手与 `Mcp-Session-Id` 被移除，每个请求在 `_meta` 中自带协议版本与客户端能力，服务端必须实现 `server/discover`；跨调用状态使用**显式 handle**（SEP-2567）；服务端发起请求改走 MRTR（Multi Round-Trip Requests）；所有结果携带 `resultType`。规范确立了 12 个月最低弃用窗口，roadmap 上还有 Tasks 扩展入 core、传输统一等持续演进项。

**结论**：协议仍在快速变化，v2 确立的"核心引擎不感知协议、MCP 仅是 Adapter 之一"的隔离决策不变；v2 深化建议中"显式 scope 句柄"的设计与规范方向一致。**具体协议条文以批次一 ADR 附权威引用为准**（规范 URL + 章节号 + 目标客户端兼容矩阵），本文只保留原则级表述，避免复制可能过时的细节。

### 2.2 SQLite edge 表 + 递归 CTE，不引入嵌入式图库

Kuzu 于 2025-10 被归档（团队被 Apple 收购），上游不再维护；Graphiti 等下游项目已陆续弃用 Kuzu 后端。v2 "Phase 0-3 仅用 SQLite"的保守决策实际上避免了一次重大返工。

**结论**：维持 SQLite-first。若 Phase 4 之后确实需要嵌入式图库，观察名单为 LadybugDB（Kuzu 社区 fork，血统最接近）、Grafeo（纯 Rust 嵌入式，最贴桌面栈）或 Lance Graph（Lance 查询层，非独立图库）——**三者成熟度与形态不可互换，届时逐个独立 ADR，不提前承诺**。

### 2.3 检索主干的证据边界（回填修订）

独立评测（arXiv 2601.07978，2026-01）显示：在 LoCoMo **对话记忆**基准上，mem0/RAG/full-context 达 77-81% 准确率，Graphiti/cognee 仅 55-56%，差距主因是检索未召回（而非推理），RAG 基线 TCO 低约 8.4 倍。

**该证据的适用边界**：LoCoMo 是多轮对话长期记忆，其图谱落后是大模型抽取的模糊实体图召回不全所致；**不能直接外推**到 Mnemex 由解析器产生的确定性结构边（调用/import/继承）。本文档引用它仅支持一个窄结论——"LLM 抽取的模糊关系图不应作为默认检索路径"。

**结论**：
- Fast Path 默认、Deep Path 按需触发——作为**产品延迟策略**保留（这是 v2 已冻结的决策），不是该论文的推论；Deep Path 增益必须由 Phase 1 内部评测集在工程记忆任务上量化验证。
- **Phase 1 的结构关联（确定性边）不受本证据影响**，仍是 MVP 核心交付物（v2 定论），进入 Fast Path 的过滤与图遍历，不放到 Deep Path 里"等评测开恩"。
- **Fast Path 相对 v2 的一处明确修订**：v2 轻量路径是"关键词 → 向量 → 直接返回"的串行结构，无融合；v3 明确为 **FTS5 与向量各取 top-k，RRF（Reciprocal Rank Fusion）融合后返回，`freshness`/`confidence` 低于阈值降权**。此修订记入第九节变更索引。

### 2.4 桌面栈

Tauri 2 + Rust + SQLite/FTS5 + 本地 GGUF 是 2026 年本地优先桌面 AI 应用中被反复验证的**可复用组合**（同类开源产品多为同构选型），构成合理默认，但"事实标准、无替代分支"的表述过强，具体组合仍以本文第六节为准。注意：该组合中本地推理实际走 llama.cpp/GGUF 路线的居多，这与第六节"embedding 用 ort、摘要用 GGUF"的分工一致。

---

## 三、选型修正

### 3.1 向量库：LanceDB 首选 + sqlite-vec 回退 + Phase 0 Spike（回填修订）

**事实**：sqlite-vec 自 2026-05-18 起无提交，稳定版停留在 0.1.9，README 自声明 pre-v1，维护风险成立；LanceDB 活跃（Rust crate v0.41.0，2026-10-29，Apache-2.0，MSRV 1.94），Argyph 等同类 Rust 项目选用。

**评审指出的原稿问题**（已回填）：用上游活跃度直接替代嵌入式适配验证，答非所问；活跃度只能淘汰 sqlite-vec 作为**默认首选**，不能证明 LanceDB 在本产品的桌面场景（双库一致性、Windows 文件锁、打包体积、增量更新）无坑。

**v3 定论**：
1. **LanceDB 定为 Phase 0 实现首选，sqlite-vec 保留为回退候选**（不设为永久弃用）。
2. Phase 0 执行一个**单候选 Spike**（约一周，中等规模仓库），四项**退出标准**写死：
   - 文件增/删/改名/分支切换时，SQLite（FTS/edge）与 LanceDB 向量的**一致性**（补偿逻辑可行）；
   - Windows 上 Tauri 打包、Defender/文件锁场景可正常运行；
   - 10 万级 chunk 增量重建的停顿可接受；
   - 备份/恢复是用户可复制的单目录操作。
   四项任一不过 → 切回 sqlite-vec 回退实现。Spike 结论写入 ADR。
3. **双存储一致性协议**为 Phase 0 设计交付物：SQLite 为 source of truth，LanceDB 可全量重建；以 chunk_id + content_hash 对齐，不做跨库事务（用"SQLite 提交 → LanceDB 追平 → 哈希校验"的补偿流）。
4. **`VectorStore` trait 抽象**为强制要求（Argyph 的 `Store` 即此模式），LanceDB 是默认实现而非边界泄漏。
5. 检索分工：**BM25 关键词检索以 SQLite FTS5 为唯一来源，LanceDB 只做向量**，RRF 在引擎层做，避免双 BM25 打架。
6. 风险表新增 **LanceDB 版本 churn**（一年 0.22→0.41 的 API 抖动与 MSRV 门槛），与 sqlite-vec 停滞对称记录。

### 3.2 代码 embedding：脱离 NC 新系，Apache 阵营候选经评测锁定（回填修订）

**事实修正**（回填）：
- v1 点名的 `jina-embeddings-v2-base-code`（约 161M）许可证为 **Apache-2.0**，本身可商用；CC BY-NC 4.0 的是新一代 `jina-code-embeddings-0.5B/1.5B`。**作废的是"jina 一类"这种会滑进 NC 权重的模糊表述与新系 jina-code，不是 v2-base-code**——后者保留为轻量对照候选。
- EmbeddingGemma-2 文本骨干 270M（完整检查点 740M 含多模态）、768 维、8K 上下文、自报 MTEB Code 78.68，分数高于 Qwen3-0.6B；但其模型卡附带 **Gemma Prohibited Use Policy 使用条款**（非纯 Apache-2.0），且 float16 会静默产出 NaN。**移出"纯 Apache-2.0 默认阶梯"**；若采用须单列 Gemma 条款、只加载文本塔、禁 float16，并以许可证审查过关为前提。

**候选矩阵（Phase 1 内部评测裁决默认档，不在基线锁死）**：

| 候选 | 规模 | MTEB Code（来源） | 许可证 | 定位 |
|---|---|---|---|---|
| Qwen3-Embedding-0.6B | 0.6B | 75.4（榜单） | Apache-2.0 | **代码+文本默认候选**（32K 上下文） |
| Qwen3-Embedding-4B | 4B | 80.1（榜单） | Apache-2.0 | 有独立显存预算时的质量档 |
| jina-embeddings-v2-base-code | ~161M | — | Apache-2.0 | 轻量对照 |
| CodeRankEmbed | 137M | CoIR 60.1 | MIT | CPU 小型代码专用对照 |
| EmbeddingGemma-2（文本塔） | 270M | 78.68（自报） | Apache-2.0 + Gemma 条款 | 许可证审查过关后参与评测 |
| Nomic Embed Code | 7B | — | Apache-2.0 | 不评估 7B 权重分发；研究竞品的 int8 静态向量蒸馏思路 |

**评测与裁决规则**：Phase 1 评测集须包含 **code embedding A/B**（候选 × Qwen3-0.6B × BM25+symbol 基线），在中英文档与代码 chunk 上同时评测；**双向量空间（文本 BGE-M3 / 代码独立）降级为待决问题**——若单一 Qwen3-Embedding-0.6B 在中英文档与代码上同时过评测线，收敛为单空间（8GB 机器避免常驻两套 embedding）；BGE-M3 的保留价值在 MIT + dense/sparse/ColBERT 三输出，采用哪种输出形态随评测结论定。

**Reranker 补位**（回填）：v2 Deep Path 含 Reranker，v3 指定候选为 **Qwen3-Reranker-0.6B（Apache-2.0）**，jina reranker 因 NC 风险不采用；Reranker 仅存在于 Deep Path。

### 3.3 本地摘要模型：Qwen3 阶梯 + 运行时分工（回填修订）

**事实**：7B 级 Q4 量化模型在 8GB 笔记本上会溢出 GPU 内存，速度崩到个位数 tok/s；Qwen3-4B Q4_K_M（约 2.5GB）在 8GB 集成显卡设备上实测约 15-18 tok/s（独立占用的 anecdotal 数据，**待设备矩阵复现**）。

**v3 定论**：
1. L2 档位修订为：**8GB 默认 Qwen3-4B Q4_K_M；16GB+ 可选 Qwen3-8B；Qwen3-1.7B 仅作测速对照**（速查表与正文保持单一清单）。Qwen3 thinking 模式默认关闭写入基线。
2. **运行时分工**（回填，修复 v1→v3 的运行时断档）：**embedding 走 ort（ONNX），摘要走 llama.cpp/GGUF**（`llama-cpp-rs` 或 FFI）——ort 不加载 Q4_K_M GGUF，v3 初稿"ort 优先、摘要用 GGUF"的表述自相矛盾，作废。
3. **风险等级维持"高"**（回填，撤销初稿降级）：15-18 tok/s 是摘要模型独占设备的数字；默认栈是 BGE-M3/Qwen3-embedding + Qwen3-4B GGUF + Tauri + LanceDB + SQLite，**embedding 与摘要争用 8GB 显存时甜点会消失**。须在 8GB 核显 / 16GB / 独显三档实测并发驻留 tok/s 与结构化抽取准确率后再议降级。
4. **L1/L2 互斥加载策略**为设计交付物：摘要进行时卸载 embedding，或强制 L2 走 CPU；16GB 开 Qwen3-8B 仅限"无并发 embedding"条件选项。
5. DesignDecision 抽取质量依赖 L2——评测集须含"决策抽取"任务类别（见 3.2 与第四节）。

### 3.4 KuzuDB 从选型表移除

Kuzu 于 2025-10-10 归档，团队被 Apple 收购（2026-02 EU 披露证实）；Graphiti 等下游已迁移。"复杂度上升后再迁移嵌入式图库"的表述修订为"届时按 2.2 节观察名单独立 ADR"。移除为整理性修订，无反对意见。

---

## 四、战略重估：索引层商品化与护城河重述

### 4.1 竞品事实（回填修正）

2026 年出现一批与 Mnemex Phase 0-2 范围重叠的开源 MCP 代码索引服务器：

| 竞品 | 技术栈 | 许可证 | 调用边解析级别 | 与 Mnemex 重叠的能力 |
|---|---|---|---|---|
| codebase-memory-mcp | C，捆绑 Nomic int8 静态向量 | MIT | 类型解析（Hybrid LSP 一类） | 语义+BM25+结构化检索、多信号融合排序、调用链 |
| codegraph | Rust + tree-sitter + SQLite/FTS5（约 20 语言），递归 CTE、PageRank、token 预算 context | 待核实 | 词法/tree-sitter | 混合检索、图遍历、context 组装 |
| Argyph | Rust + tree-sitter + LanceDB + 本地 embedding | Apache-2.0 | tree-sitter | 符号图 + 混合检索 + repomix 式打包 |
| open-codebase-index | SQLite + usearch + BM25 | 待核实 | tree-sitter | branch-aware 增量索引、call graph |
| Serena、GitNexus、repowise 等 | live LSP / LadybugDB 等 | 部分为 AGPL | live LSP | 索引与检索（**AGPL 依赖须提前排除**） |

**两点修正**（回填）：其一，初稿"codegraph 工具面几乎等于 Phase 2 计划的 MCP tool 清单"不成立——v1/v2 的 Phase 2 tool 是 `search_*` / `get_related` / `get_summary` / `get_context_bundle`，PageRank/impact 并不在其中；竞品能力是对**索引面**的覆盖，不是对**记忆面**的覆盖。其二，真正要正视的差距在**调用边的解析级别**：竞品已做到类型解析级调用边，仅靠 tree-sitter 名字匹配做"结构关联"会产生大量误连边，而"让 Agent 理解项目"恰恰建立在这些边上。**Phase 1 结构图谱的验收标准：目标语言上调用边达到可解释精度，并在回归集单列**；类型解析可以后做，但不得在"够用即可"里被默默降级为名字匹配。

### 4.2 护城河重述（回填）

"索引不是护城河，记忆才是"成立，但护城河的准确表述不是"别人没有 Knowledge Object"，而是：**工程知识对象与工作区新鲜度（freshness）、commit 绑定（CommitEvent）、矛盾检测（contradiction_score）绑在一起，且文档+代码统一建模**——免费索引器不做这些，通用记忆库不懂这些。

### 4.3 薄 / 厚清单（取代初稿"压薄"表述）

| 薄（对齐竞品即可，明确不追平） | 厚（必须超过免费索引器） |
|---|---|
| 语言数量：MVP 只做 Java / Rust / TS / Python 四门 | Knowledge Object 模型与生命周期字段 |
| MCP tool 个数：维持 v2 那一组，不加 PageRank/impact | DesignDecision / CommitEvent 抽取质量 |
| LSP 级类型解析：后置，不阻塞 MVP | freshness / contradiction 的可解释更新 |
| IaC、32 语言、162 grammars 等覆盖面 | 文档+代码统一对象，而非纯代码图 |

Phase 1 的定位由此改为**"记忆对象 MVP"**，而非"压薄的 Phase 1"——它在索引面上收敛（不做覆盖面追平），在记忆面上是全量投入。Phase 3-4 **维持数据闸门**：没有真实使用记录（usage_count 数据），不投入主动提醒调参与跨项目画像挖掘；把资源提前砸向 Phase 4 等于把 v2 刚收敛掉的范围从另一头加回来。

### 4.4 "借力 vs 全自研"决策门（回填）

解析/索引层在 **Phase 0 结束前二选一**，输出一页 ADR：自研薄索引，或包装外部索引器（Apache/MIT 许可）。关键判断：索引器的图是符号图，Mnemex 的核心是 Knowledge Object，**阻抗在数据模型映射，不在许可证**；嵌入 C/TS 索引器的映射成本与自研 Rust 管线的成本须在同一 spike 里对比。竞品功能矩阵作为 Phase 1 交付物时，必须带仓库、许可证、提交日期三列。

### 4.5 内部评测集（回填：定位收窄）

- **定位：回归门禁**——"调整分块策略/排序权重后不得回退"，不承担"记忆质量优于免费索引器"的对比证明；竞品对比（如做）留待 Phase 2，且必须同仓库、同 query 集。
- **任务分类与下限**：结构查询（symbol/调用边）、决策召回、过时检测三类，每类不少于 15 条（确切 N 在详细设计中定）；区分冒烟集与回归集。
- 评测集同时承载 3.2 的 code embedding A/B 与 3.3 的决策抽取质量验证。

---

## 五、MCP Adapter 基线：dual-era（回填 blocker 修订）

### 5.1 双版本兼容（撤销初稿 modern-only）

**事实**：2026-07-28 规范已定稿无状态核心，但主流客户端（Cursor、Claude Desktop 等）在 2026-09/10 仍发送 2025-11-25 的 `initialize` 握手；生态探测显示现网 MCP 服务器普遍 dual-era，无 modern-only 先例；规范兼容矩阵写明 legacy client × modern-only server = Fails，且 legacy 客户端无前向回退。

**v3 定论**：
- Adapter 实现 **2026-07-28 无状态路径为主路径**（`server/discover`、`_meta` 版本协商、无会话），**同时保留 `initialize` 兼容旧客户端**；legacy 路径的拆除日期与规范 12 个月弃用窗口及目标客户端兼容矩阵对齐，**不在 Phase 2 拆除**。
- 基线补齐初稿遗漏的契约项：所有响应携带 `resultType`；需要客户端补充信息时走 MRTR；`roots/list_changed` 等已弃用通知不再依赖。
- 引擎内部只保留一套无状态调用语义，两版本差异全部封在 Adapter 内。

### 5.2 scope 参数与 handle 的正确分工（回填，撤销"直接映射"）

- **Client-supplied scope**：工作区路径 + 可选分支/提交（v2/深化建议中的 `scope_id`）保持为**普通工具参数**，每次调用由客户端显式传入——这是无状态查询的默认形态，不额外铸造。
- **Server-minted handle**：仅用于客户端构造不出的**真正服务器状态**——索引快照、分页游标、Phase 3 订阅、Tasks 任务句柄。handle 的铸造/过期/作废规则写入 Adapter ADR，不进入 Memory Engine。
- 初稿"scope_id 直接映射为 server-minted handle"作废：那会给无状态查询平添一轮往返，把会话状态换个名字请回来。

### 5.3 推送与 Phase 3（回填）

- **事实修正**：Tasks 已在 2026-07-28 成为**官方扩展**（`io.modelcontextprotocol/tasks`），不存在"待转正"；`subscriptions/listen` 已落地，但承载的是工具/资源列表类变更通知，不是"当前编辑文件与旧决策相关"这类语义推送。
- **Phase 3 主路径**：本地 UI 角标 + 拉取式 tool（轮询/HTTP 长轮询兜底），**不自建语义 push**；MCP Tasks 用于可取消的长任务（全量索引、批量摘要），subscriptions 通道作为后续增强。Phase 3 可拆 3a（拉取式，承诺交付）/ 3b（规范推送通道，可选加速）。
- **HTTP API 与 MCP Adapter 维持分离**（回填，撤销"借 `Mcp-Method`/`Mcp-Name` 头合并"的评估方向）：两者只共享 Engine 接口；合并会把产品 JSON API 绑上 `resultType`/`_meta` 等协议语义，与 2.1 的隔离结论相反。

---

## 六、技术选型速查（v3 修订版 · 含许可证列）

| 层级 | 选型 | 许可证 | 较 v2 的变化 |
|---|---|---|---|
| 文件监控 | Rust `notify` + 定时全量兜底 | MIT/Apache | 不变 |
| 文档解析 | docx-rs / calamine / pdf-extract / lopdf | 各异（详见 ADR） | 不变 |
| 代码解析 | tree-sitter，MVP 四语言（Java/Rust/TS/Python），调用边验收见 4.1 | MIT | **MVP 语言集显式化** |
| 文本+代码 embedding | Qwen3-Embedding-0.6B 默认候选（单/双空间待评测裁决，见 3.2）；BGE-M3 为混合检索备选 | Apache-2.0 / MIT | **替换初稿"双空间维持不变"** |
| 推理后端（embedding） | ort（ONNX Runtime） | MIT | 与摘要分工 |
| 推理后端（摘要） | **llama.cpp（`llama-cpp-rs`/FFI）跑 GGUF** | MIT | **回填 v1 的运行时，修复与 ort 的矛盾** |
| 本地摘要模型（L2） | 8GB 默认 Qwen3-4B Q4_K_M；16GB+ 可选 Qwen3-8B；1.7B 仅测速对照；thinking 关闭 | Apache-2.0 | **替换 Qwen2.5-7B，运行时闭环** |
| Reranker（Deep Path） | Qwen3-Reranker-0.6B 候选 | Apache-2.0 | **新增行**（v2 遗留空位） |
| 向量库 | **LanceDB 首选（`VectorStore` trait 后可换）**，sqlite-vec 回退；Phase 0 Spike 四项退出标准 | Apache-2.0 / MIT+Apache | **"二选一"改为"首选+回退+验证"** |
| 关键词检索 | SQLite FTS5（唯一 BM25 来源），RRF 在引擎层 | 公有领域 | 显式化 |
| 图存储 | SQLite edge 表 + 递归 CTE | 公有领域 | **KuzuDB 出局** |
| Agent 集成 | MCP Adapter（**dual-era**：2026-07-28 主路径 + initialize 兼容）+ 本地 HTTP/JSON API（独立 Adapter） | — | **初稿 modern-only 作废** |
| CLI 适配 | **延后至 Phase 2 末可选交付**（调试用途），非承诺 | — | **显式记录 v2→v3 变化** |

---

## 七、路线图修订（v3 · 薄索引面 / 厚记忆对象）

| Phase | v3 交付物 | 较 v2 的变化（显式声明） |
|---|---|---|
| Phase 0 | 文件扫描 + Git 分析 + 四语言 AST 解析 + SQLite/FTS5 基础设施 + **LanceDB Spike（四项退出标准）+ 双库一致性协议设计 + `VectorStore` trait + 机器档位矩阵实测（8GB 核显/16GB/独显）** | **范围变化**：v2 Phase 0 "不含向量检索"，v3 将向量验证以 Spike 形式提前进入 Phase 0——这是对 v2 的显式修订，非笔误；Spike 不过则按 3.1 回退 |
| Phase 1 | **记忆对象 MVP**：Knowledge Object 建模 + 生命周期字段、DesignDecision/CommitEvent 抽取（含事件追加式存储，即 v1 knowledge_events 表形态并入此阶段；面向个人的历史查询 tool 仍留 Phase 3）、四语言调用边（可解释精度验收）、项目摘要、结构关联图谱 + **内部评测集（回归门禁，三类任务各 ≥15 条）** + **竞品功能矩阵（含仓库/许可证/提交日期/解析级别四列）** + 借力 vs 自研 ADR（Phase 0 末产出，Phase 1 生效） | 新增两个硬交付物；索引面收敛（语言四门、不做 PageRank/impact 追平）；评测集定位为回归门禁 |
| Phase 2 | Memory Engine 核心 + **dual-era MCP** + HTTP 双适配 + Context Bundle；MCP tool 维持 v2 清单（`search_semantic`/`search_symbol`/`get_related`/`get_summary`/`get_context_bundle`），差异字段（confidence/freshness/contradiction_score）随结果暴露 | CLI 延后（见第六节）；**先把带生命周期字段的 MVP 装到自己的仓库上跑出真实 usage_count**，这是 Phase 3-4 的数据前提 |
| Phase 3 | 主动关联提醒：3a 本地角标 + 拉取式 tool（承诺）；3b 规范推送通道（可选） | push 机制依赖修订（见 5.3） |
| Phase 4 | 跨项目模式挖掘、个人技术风格画像 | **维持数据闸门**：无真实使用记录不开工 |

**资源原则修正**（回填）："把省下的投入全部给 Phase 3-4"作废——Phase 3-4 依赖真实使用数据，提前开工只会重演 v2 刚收敛掉的范围膨胀。正确顺序：薄索引面 → 尽快把记忆 MVP 装到自己仓库上 → 用真实数据驱动 Phase 3-4。

---

## 八、风险清单（v3 更新 · 回填后）

| 风险 | 等级 | 变化 | 应对 |
|---|---|---|---|
| 范围膨胀 | 高 | 不变 | 维持 v2 定位收敛 + 本节 4.3 薄/厚清单 |
| 免费竞品功能面压制 | **高** | 新增 | 竞品矩阵（含解析级别列）+ 评测集回归门禁 + 差异字段暴露 |
| **调用边精度**（tree-sitter 名字匹配误连） | **高** | **新增（回填）** | Phase 1 四语言调用边可解释精度验收，回归集单列；类型解析后置但有明确路线 |
| **本地资源预算**（8GB 上 embedding+摘要并发驻留、双库、GGUF 运行时） | **高** | **新增（回填，承接 v2"本地模型性能"）** | 设备矩阵实测 + L1/L2 互斥加载策略；实测过关前不降级 |
| 本地模型性能 | 高 | **维持"高"（撤销初稿降级）** | 同上 |
| **双库一致性**（SQLite↔LanceDB 无跨库事务） | **中-高** | **新增（回填）** | source of truth + 重建协议 + Spike 退出标准 |
| **LanceDB 版本 churn / MSRV** | 中 | **新增（回填）** | `VectorStore` trait 隔离 + 依赖升级窗口策略 |
| 检索质量 | 高 | 不变 | 评测集回归门禁；Deep Path 增益须量化验证 |
| 数据污染 | 中 | 不变 | Memory 生命周期字段 |
| MCP 协议变化 | 中 | 不变 | dual-era Adapter + 12 个月弃用窗口跟踪 + 客户端兼容矩阵 |
| **Windows 桌面实机**（文件锁、Defender、长路径、tree-sitter 原生构建体积） | 中 | **新增（回填）** | Phase 0 Spike 第 2 项 + 三平台冒烟 |
| **模型资产体积/离线安装** | 中 | **新增（回填）** | 首次下载体验与模型分发策略入详细设计 |
| 依赖维护状态 | 中 | 新增 | 选型时检查上游活跃度（commit/release/advisory），sqlite-vec/Kuzu 事件作为流程化教训 |
| **AGPL 依赖混入**（若开源） | 低-中 | **新增（回填）** | 竞品矩阵许可证列；依赖引入前 SPDX 审查 |

---

## 九、与此前文档的关系（完整变更索引，回填）

| 来源文档 / 章节 | v3 动作 |
|---|---|
| v2 §一 定位收敛 / §二 Knowledge Object / §五 Memory 生命周期 / §六 Engine-Adapter 解耦 | **维持**，趋势验证 |
| v2 §三 图谱策略（"复杂度上升后再迁移嵌入式图库"） | **修订**：Kuzu 移除，观察名单独立 ADR（见 3.4、2.2） |
| v2 §四 检索链路轻量路径（关键词→向量串行） | **修订**：Fast Path 增加 RRF 融合与生命周期降权（见 2.3） |
| v2 §四 Deep Path（含 Reranker） | **维持并补位**：Reranker 指定 Qwen3-Reranker（见 3.2） |
| v2 §七 AI 分级 L2（3B-7B） | **修订**：Qwen3 阶梯 + 运行时分工（见 3.3） |
| v2 §八 Phase 0（不含向量检索） | **修订**：向量 Spike 提前（见第七节，显式声明） |
| v2 §八 Phase 2（MCP/HTTP/CLI 多适配） | **修订**：MCP dual-era + HTTP 独立；CLI 延后（见 5.1、第六节） |
| v1 §五 技术选型速查 | **整体取代**（本文第六节）；其中 jina-embeddings-v2-base-code 条目**修正保留**（Apache-2.0，见 3.2） |
| v1 §三 时间维度 knowledge_events | **澄清**：事件追加式存储随 CommitEvent 并入 Phase 1；个人历史查询 tool 留 Phase 3（见第七节） |
| v1 隐私架构（目录级"禁止出云"硬约束） | **维持并重申有效**，v3 未修订 ≠ 作废 |
| 深化建议"显式 scope 句柄" | **维持**，语义修正为 client-supplied 参数（见 5.2） |
| 深化建议"双协议暴露" | **维持**（HTTP API 与 MCP 分离，见 5.3） |
| 《详细设计阶段输出优先级结论》批次一 ADR | **扩充**：原四项选型 ADR 扩为——①向量库首选/回退与退出标准 ②双库一致性协议 ③embedding 选型与许可证审查 ④摘要运行时与 L2 档位 ⑤Kuzu 移除 ⑥MCP dual-era 与 handle 语义 ⑦Fast Path RRF ⑧Phase 1 回归集门禁 ⑨借力 vs 自研裁决。**评审闭合后立项，ADR 按本文回填后的结论写** |

---

## 十、三模型交叉评审结论与回填记录

**评审流程**：v3 初稿完成后，由三个不同模型的独立评审代理（评审方 A / B / C）在互不可见的情况下对同一文档评审，维度为技术合理性 / 内部一致性 / 完备性 / 战略可落地性。

**三方结论**：A 有条件通过 6.5/10；B 有条件通过 6.5/10；C 有条件通过 6/10。意见收敛度高，共性结论如下：

| 共性发现（severity） | 三方命中 | 处置 |
|---|---|---|
| MCP modern-only（"不实现 initialize"）与主流客户端不兼容 | A major / B blocker / C blocker | **已回填**：5.1 dual-era，补 resultType/MRTR/弃用项 |
| v2 Phase 0"不含向量检索"与 v3 LanceDB 落地矛盾且未声明 | A blocker / B major / C blocker | **已回填**：第七节显式声明为对 v2 的修订 |
| EmbeddingGemma-2 许可证非纯 Apache-2.0 且档位排序与分数矛盾 | A blocker / B major / C major | **已回填**：3.2 移出默认阶梯，单列 Gemma 条款 |
| jina-embeddings-v2-base-code（Apache-2.0）被 CC BY-NC 结论误伤 | B major / C major | **已回填**：3.2 事实修正 |
| LanceDB"定稿并取消一切验证"过猛；双库一致性未设计 | A major / B major / C blocker | **已回填**：3.1 首选+回退+Spike 四项退出标准+一致性协议 |
| GGUF 摘要模型与 ort 推理后端矛盾 | B major / C major | **已回填**：3.3 运行时分工 |
| "本地模型性能"风险降级证据不足（并发驻留未测） | A minor / B major / C major | **已回填**：风险维持"高"，设备矩阵实测为前置 |
| LoCoMo 对话记忆证据被外推到代码检索 | A major / B major / C major | **已回填**：2.3 收窄证据边界，结构边仍为 Phase 1 核心 |
| "Phase 1-2 压薄"与实际新增交付物自相矛盾 | A major / B major / C major | **已回填**：4.3 薄/厚清单，Phase 1 改名"记忆对象 MVP" |
| scope_id 误映射为 server-minted handle | B major / C major | **已回填**：5.2 分工修正 |
| "Tasks 待转正"表述过时；subscriptions 非语义推送 | B major / C major | **已回填**：5.3 事实修正，Phase 3 拆 3a/3b |
| §九 修订范围声明小于实际变更面 | A major / B major / C major | **已回填**：第九节改为完整变更索引 |
| 竞品表事实问题（codegraph 描述混行、"几乎等于"过度声明、缺许可证列） | A minor / B minor / C major | **已回填**：4.1 修正 + 新增解析级别维度 |
| 调用边解析级别差距（tree-sitter 名字匹配 vs 类型解析）未入风险 | C major / A 隐含 | **已回填**：新增高风险行 + Phase 1 验收标准 |
| 借力 vs 自研无决策门 | A major / B major / C major | **已回填**：4.4 Phase 0 末二选一 ADR |
| Windows 桌面实机风险缺失 | A major / B 完备性 / C Spike 隐含 | **已回填**：Spike 第 2 项 + 风险表 |
| 隐私"禁止出云"/ knowledge_events 在 v3 中静默消失 | A minor / B 完备性 | **已回填**：第九节重申/澄清 |
| 双向量空间"维持不变"应改为待决问题 | B major / C major | **已回填**：3.2 待评测裁决 |

**未采纳 / 部分采纳的个别意见**：A 建议完全撤销风险降级——已按"维持高"采纳；B 建议 Phase 0 默认 embedding 改 Gemma-2——因许可证条款未审查，改为"评测候选参与，默认档由评测+许可证双过关裁决"，与 A 的 SPDX 审查要求合并处理；C 建议 CLI 从 Phase 2 移除——采纳"延后至 Phase 2 末可选交付"折中。

**定稿条件**：本版已回填全部 blocker 与主要 major 意见；剩余 minor/nit（引用脚注化、个别数字标注来源日期）随批次一 ADR 落实。**本文档现为 Architecture Baseline v3。**

---

*本文档基于 2026-11 开源生态调研修订，经三模型交叉评审回填后定稿。*
