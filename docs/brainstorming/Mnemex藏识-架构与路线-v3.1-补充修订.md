# Mnemex / 藏识 — 架构与路线 v3.1（2026-11 补充修订 · 三模型交叉评审回填版）

> 本文基于 2026-11 下半月（H2 趋势）的第二轮调研，对《架构与路线 v3（2026-11 开源趋势修订 · 交叉评审回填版）》做补充修订，作为当前实现基线。v3 及更早文档保留为历史记录，本文与 v3 冲突之处以本文为准；v3 中未被本文触及的结论继续有效。
>
> **修订流程**：初稿由三个不同模型的独立评审代理交叉评审（评审方 A / B / C），三方结论均为"有条件通过"（7.0 / 6.4 / 6.5 分）。本版已回填全部 blocker 与主要 major 意见，回填记录见第十二节。
>
> **修订动因**：① Qwen3.5 家族（2026-02/03，全系 Apache-2.0）与 Gemma 4 转向 Apache-2.0（2026-03-31）；② Rust 推理生态明确"OpenAI 兼容本地服务"模式；③ AGENTS.md / auto-memory / Skills 成为跨厂商事实标准；④ GPUI 独立桌面应用可行性完成评估；⑤ Jev AI 一类"结构化决策"服务点破 v3 AI 分级中 L0 与 L2 之间的空档。

---

## 一、修订范围总览

| # | 修订项 | 性质 | 影响 |
|---|---|---|---|
| 1 | L2 摘要模型升级 Qwen3.5 阶梯 | 选型更新 | 替换 v3 的 Qwen3-4B/8B |
| 2 | 推理运行时改为 OpenAI 兼容 sidecar（MVP 绑定 llama.cpp server 单后端） | 架构优化 | 替换 v3 的"进程内 llama-cpp FFI"默认路径 |
| 3 | 新增 L1.5"结构化决策层"（Phase 2 影子运行 + 门禁切换，不接管护城河字段） | 架构优化 | v3 AI 分级五级化，相位经评审修正 |
| 4 | 新增 Agent 工件通道（拆分为只读采集源 + 显式授权导出器） | 产品方向 | 读写分离，见第五节 |
| 5 | Context Bundle 增加"compaction 稳定性"约束 | 设计约束 | Phase 2 交付物补充 |
| 6 | 轻量模型评测池收敛扩充 + Provider 分层 + MRL 政策修正 | 候选矩阵扩充 | v3 §3.2 增补 |
| 7 | GPUI 评估：维持 Tauri，观察记录（不占决策 ADR 编号） | UI 决策 | UI 规范不变 |
| 8 | 扫描 PDF：默认只走文本层，视觉转写为显式增强 | 管线策略 | 修正初稿"视觉优先" |

## 二、L2 摘要模型：Qwen3.5 阶梯（替换 v3 §3.3 档位）

**事实**：Qwen3.5 家族全系 Apache-2.0；Qwen3.5-4B（4.2B 参数）Q4_K_M 约 **3.3GB**（注意：比 v3 的 Qwen3-4B 大 0.8GB，资源预算须重估），**262K 原生上下文**，视觉+工具调用+thinking 三合一；Qwen3.5-9B 摘要质量接近云端上代模型（26 tok/s 数据来自 Ryzen AI Max+ 级 iGPU，不能外推为 8GB 档）。

**v3.1 定论**：

| 档位 | 模型 | 说明 |
|---|---|---|
| 8GB 默认 | **Qwen3.5-4B** Q4_K_M | thinking 关闭；`n_ctx` 设远小于 262K 的硬顶（KV cache 在 8GB 上会挤占权重） |
| 16GB+ | Qwen3.5-9B | 仅限无并发 embedding 条件 |
| 轻量/测速 | Qwen3.5-2B / 0.8B | 0.8B/2B 兼任 L1.5 决策层主力（见第四节） |

**配置策略（回填修正）**：thinking 默认关闭 + 输出 token 上限——**类型上必填，但按任务给预设默认值**（随产品配置下发：摘要/抽取/决策/视觉各一档），缺预设则拒绝启动；"必填但无默认"会把事故从运行时搬到调用点，作废。回归集增加"thinking 误开"与"预设缺失"用例。

**资源预算重估（回填）**：4B 从 2.5GB→3.3GB、新增 L1.5 常驻、推理改独立进程、视觉任务图 token 远重于摘要——v3 的"L1/L2 互斥"升级为**进程级排程**：同一时刻仅驻留 embedding / L1.5 / L2 之一，切换时卸载（见第四节正交分级表）。Phase 0 设备矩阵按新栈**重测**，v3 的旧矩阵数据作废；退出标准增加：互斥调度下 tok/s、L1.5 每 chunk 延迟、峰值内存。不过线则 8GB 默认降至 2B 并关闭视觉。

## 三、推理运行时：OpenAI 兼容 sidecar（MVP 单后端，回填修正）

**v3.1 定论**：
- `SummaryProvider` / `DecisionProvider` 本地默认实现 = OpenAI 兼容客户端 + 本地推理 **sidecar**；进程内 FFI 后置为可选低延迟路径（仅当 L1.5 评测过线且延迟不达标时提前）。
- **MVP 绑定单一后端：llama.cpp server**（回填：三后端在受约束解码上并不互通，"换 endpoint 即可"不成立）。Ollama 仅作**探测用户已有实例**的可选端点——支持受约束解码才注册为 `DecisionProvider`，否则仅作 `SummaryProvider`；mistral.rs 留观察名单（bus factor）。
- **协议边界澄清（回填）**："OpenAI 兼容同一协议"覆盖 **L2 生成与 L3 云端**；受约束解码（GBNF/JSON schema）是 **provider 私有能力的协商项**，GBNF 是 llama.cpp 适配器的内部实现，不泄漏进引擎。`DecisionProvider` 契约 = **JSON schema 进、类型出、失败回退 L0**（fail-closed）。
- **sidecar 生命周期管理最小子集前移至 Phase 0 退出标准**（回填：与 LanceDB Spike 并列）——进程生命周期、端口冲突、僵尸进程处理，避免 Phase 2 才首次集成推理。
- **出云边界（回填，评审方 C 独有发现）**：本地与 L3 共用 OpenAI 兼容客户端后，出云边界从"不同实现"退化为 `base_url` 字符串。共享 HTTP 客户端**默认仅允许 loopback**；非 loopback 必须同时满足：L3 总开关开启 + 路径不在"禁止出云"列表 + UI 二次确认。风险表新增"误配 base_url 出云"（中-高）。

## 四、L1.5 结构化决策层（相位经评审修正）

**动因**（源自 Jev AI 调研）：v3 四级分级中，高频流水线决策卡在 L0（脆）与 L2（重）之间。Jev AI（云端 `/v1/decide`，RLCD 校准置信度）代表的"廉价、校准、结构化 System 1 决策"类别与 Mnemex 流水线任务重合。

**三轴正交分级表（回填，消灭"L*"语义重叠）**：

| 轴 | 级别 | 说明 |
|---|---|---|
| 推理能力 | L0 规则 / **L1 embedding（ort）** / L1.5 受约束解码 / L2 生成 / L3 云端 | 任务复杂度自动选择（v2 决策延续） |
| 模型分发 | 默认下载层（Apache/MIT only）/ BYO 层（用户自带）/ 云端层（显式开启） | 许可证与来源分层 |
| 隐私 | 目录级"禁止出云"硬约束 | 维持 v1/v3，与上面两轴**正交而非同构** |

（"Provider 三分层与隐私 L0-L3 同构"的表述作废——只是修辞相似，实现时会串权限判断。）

**相位与切换门禁（回填 blocker 修正）**：

| 阶段 | L1.5 职责 |
|---|---|
| Phase 1 | **不存在**。抽取/打分维持 v3 分工：L0 启发式 + L2——与 v3 Phase 1"记忆对象 MVP"硬交付无裂缝 |
| Phase 2 上线 | 先做 **chunk 类型分类** + **影子运行**（只记日志、不挡入库、不写库）；回归集门禁 = **决策抽取质量不低于纯 L2** 且 8GB 上 chunk 分类延迟达标 |
| 门禁通过后 | 才把 DesignDecision 三元组预筛、BugRecord 识别、提醒价值评分切至 L1.5；切换前 **DesignDecision 的落库者始终是 L2** |
| 路线图外后置项 | 置信度校准（采纳/拒绝反馈 → 温度缩放/等渗回归）、LoRA——v3 路线图只有 Phase 0-4，"Phase 6"表述作废 |

**置信度语义三层拆分（回填 blocker 修正）**：
- `schema_valid`（JSON 合法）≠ 原始口头分/token 概率 ≠ 校准后 `confidence`；
- **KO 字段拆为 `model_confidence`（原始）与 `confidence`（校准后）**；v3 Fast Path 的"`confidence` 低于阈值降权"**只消费校准值**，校准门禁通过前该降权**关闭**，排序只用确定性信号；
- `freshness` **保持确定性规则**（git/hash/mtime），不走模型打分（回填：可解释性是"厚记忆"的验收标准）；`contradiction_score` 仅对近邻候选（同符号/同文件/向量近邻）调用 L1.5 并单列预算。

**级联路由（回填修正）**：L0 能判定的不调模型 → L1.5 出候选 → **低分或 schema 失败升 L2**（初稿"复核高置信样本"方向反了，作废）；校准前高分不得单独落库。

**Jev AI 处置**：不进默认链路；L3 云端增强层可选 Provider 观察名单；命中"禁止出云"被硬拦（loopback 默认见第三节）。

## 五、Agent 工件通道（读写分离，回填修正）

**事实**：AGENTS.md 已成跨厂商仓库指令事实标准（60,000+ 项目，Codex/Cursor/Copilot/Windsurf/opencode 原生读取，Claude Code ≥2.1.277 回退读取）；Claude Code auto-memory 为 typed 笔记（user/feedback/project/reference）按仓库持久化；Skills（SKILL.md）为惰性加载开放标准。

**v3.1 定论（拆为两个组件，不再合并为一个"Adapter"）**：

1. **`AgentArtifactSource`（Phase 1，只读采集源）**：默认仅读取**仓库内**的 AGENTS.md / CLAUDE.md / SKILL.md，按普通 Markdown 解析（不解析 auto-memory 的厂商目录格式，格式漂移隔离在采集层）。auto-memory 导入为 **Phase 2 opt-in**，目录布局以 ADR 钉住，不进 Phase 1 关键路径。
2. **`SkillExporter`（Phase 4，显式授权导出器）**：模式库 → SKILL.md/AGENTS.md 片段；**默认关闭**；默认写入用户本地技能目录（非 git 仓库——个人画像不自动进 git）；写入仓库须单次显式确认。不自动 commit。

**Adapter 承诺计数修正（回填）**：当前承诺 = **MCP（dual-era）+ HTTP + Agent 工件** 三件；CLI 维持 v3 的"Phase 2 末可选交付"，不计入承诺数（初稿"MCP/HTTP/CLI/Agent 工件四个 Adapter"计数作废）。

**Context Bundle compaction 稳定性（Phase 2 验收，回填修正）**：引用以 **KO id + content_hash + commit 为主、路径为辅**（路径引用在 compaction 后可能指向新内容，绕开 freshness 绑定）；验收两条——compaction 后仅存 ID 仍能二次 `get_context_bundle` 拉取；截断重注入不改变稳定 ID。不绑死 Claude `/compact` 的具体行为。

## 六、轻量模型与评测政策（回填收敛）

**Phase 1 评测裁决只新增两席**（回填：评测是回归门禁不是打榜，一次加五个候选会矩阵爆炸）：

| 新增席位 | 规格 | 许可证 | 定位 |
|---|---|---|---|
| gte-modernbert-base | 149M，8K，CoIR 79.31 | Apache-2.0 | 轻量档对照（与 Qwen3-0.6B、BGE-M3 同维政策比） |
| gte-reranker-modernbert-base | 149M，LoCo 90.68 | Apache-2.0 | 低端机 Deep Path reranker；**低端档默认无 reranker**（Deep Path 关闭时不下载） |

**BYO 名单（不进默认裁决）**：nomic-embed-text-v1.5（137M，Apache-2.0，全开源可审计；英文为主）、jina-embeddings-v5 系（CC BY-NC，质量优秀但商业授权由 Elastic 销售）。**jina-embeddings-v2-base-code（Apache-2.0）显式保留为轻量对照**（回填：防止再次与 NC 新系混淆——v3 评审已踩过一次）。

**排除维持**：mxbai-embed-large-v1——排除标准改写为通用规则：**"chunk 上限超过 512 token 的模型不入池"**，并在同一 ADR 写死 chunk 上限（回填：排除理由从"某模型"升格为入池规则）。

**MRL 政策修正（回填）**：
- 向量表维度**建表时固定**；改维度 = 全量重嵌，与换模型同级（"截维度不用重建"的表述作废——那只对"全维入库、查询时截断"成立）；
- `VectorStore` 交付物必须写明**存储维 vs 查询维**两个概念，维度写入索引元数据；
- 默认**全维存储**，截断是低端机的显式配置；"可截到 32 维"不作为产品能力宣传，代码/中文检索设下限档（256/512）；
- BGE-M3 固定 1024 维作为**对比说明**，评测前不构成淘汰扣分。

**Gemma 4 处置（回填，消除动因悬空）**：ADR③ 新增观察候选条目——Gemma 4 系（2026-03-31 起 Apache-2.0），**许可证复审确认附加条款撤销后**方可进入默认下载层与评测池；EmbeddingGemma-2 维持 v3 的 SPDX 审查处置不变。

## 七、GPUI 评估：维持 Tauri，观察记录

**事实**：GPUI 独立桌面应用可行（Windows 走 Win32 + DirectWrite），但 pre-1.0 高频破坏性变更、本体零 widget（第三方 gpui-kit 补齐）、Windows 后端最年轻且 IME/无障碍差距明显、无热重载；生产案例仅 Zed / Longbridge Pro / Cap。

**v3.1 定论**：**UI 维持 Tauri 2，不切换 GPUI**（三重理由：Windows 中文 IME 硬伤；液态玻璃视觉语言是 Web 原生优势；UI 是"薄"面，不烧 pre-1.0 churn）。**以观察记录形式跟踪（不占决策 ADR 编号）**，重评估触发条件（三条同时满足）：GPUI 1.0 且 Windows IME/无障碍补齐；gpui-kit 独立稳定发布；WebView2 渲染玻璃效果在目标机型出现**可测的性能问题**（阈值示例：1080p 主面板交互 30s 内 GPU 占用/掉帧超阈值——写成可测条件，否则观察项永不触发也无需触发）。

## 八、扫描 PDF 策略（回填修正：视觉不进默认路径）

**v3.1 定论**：
- **默认**：仅提取文本层（pdf-extract/lopdf，v3 不变）；判定为扫描页（无文本层/文本过短）则**标记并跳过**，不阻塞索引；
- **视觉转写为 Phase 2+ 显式增强**：用户显式触发，使用独立的输出 token 预设（不得套用摘要的短上限，否则正文截断入库）；设每文档页数上限 + 图像最长边限制 + 超时回退 OCR 兜底；独立 OCR 保持 Phase 2 可插拔，**不在基线写死"视觉优先"**（初稿表述作废——VLM 读中文密集表格/代码截图质量常不如专用 OCR）；
- 视觉不进 Phase 0 冒烟范围；风险表新增"视觉解析拖垮索引 Job"（中）。

## 九、技术选型速查（v3.1 增量）

| 层级 | v3.1 选型 | 较 v3 的变化 |
|---|---|---|
| 本地摘要模型（L2） | Qwen3.5-4B 默认 / 9B / 2B / 0.8B；thinking 关；token 上限按任务预设、缺预设拒启动 | **替换 Qwen3-4B/8B** |
| 结构化决策（L1.5） | Qwen3.5-0.8B/2B + 受约束解码；Phase 2 影子运行 + 门禁切换；失败回退 L0 | **新增层（相位修正）** |
| 推理运行时 | OpenAI 兼容 sidecar，**MVP 绑定 llama.cpp server**；Ollama 探测式可选端点；FFI 后置 | **替换默认路径（单后端）** |
| 轻量 embedding | 评测池 +gte-modernbert-base / +gte-reranker；nomic/jina v5 进 BYO | 收敛扩充 |
| Agent 工件 | AgentArtifactSource（Phase 1 只读）+ SkillExporter（Phase 4 显式授权） | **读写分离** |
| OCR | 默认文本层；扫描页标记跳过；视觉转写 = Phase 2+ 显式增强，OCR 可插拔兜底 | **修正"视觉优先"** |
| UI | Tauri 2（GPUI 观察记录，三条可测触发条件） | 确认维持 |

## 十、路线图增量（相位修正版）

| Phase | v3.1 增量交付 |
|---|---|
| Phase 0 | +sidecar 选型冒烟（**绑定 llama.cpp server**）+ **sidecar 生命周期最小子集入退出标准** + 设备矩阵按新栈重测（互斥调度 tok/s、L1.5 每 chunk 延迟、峰值内存） |
| Phase 1 | +AgentArtifactSource 只读（仓库内 AGENTS/CLAUDE/SKILL.md 按普通 Markdown）；抽取维持 L0+L2（无 L1.5）；评测池新增两席 |
| Phase 2 | +L1.5 上线（chunk 分类 + 影子运行）；切换门禁（不低于纯 L2 + 延迟达标）；+auto-memory opt-in 导入；+compaction 稳定性验收（两条）；先狗粮跑出 usage_count——**Phase 2 承诺保持为引擎 + 双 Adapter + 带生命周期字段的 Bundle，L1.5 切换不阻塞狗粮闸门** |
| Phase 3 | 提醒价值评分走 L1.5（若门禁已过） |
| Phase 4 | SkillExporter（显式授权，默认写本地技能目录） |
| 路线图外后置项 | 置信度校准 → LoRA（依赖用户反馈数据积累） |

## 十一、风险清单增量（回填后）

| 风险 | 等级 | 说明 |
|---|---|---|
| 误配 base_url 出云 | **中-高** | 共享 HTTP 客户端默认 loopback；非 loopback 三重门（L3 总开关 + 禁止出云列表 + UI 确认） |
| L1.5 未校准分数污染排序 | 中 | `model_confidence` 与 `confidence` 分离；校准前降权关闭；回归集加"口头分 vs 人工标签"可靠性曲线 |
| Qwen3.5 thinking 零输出 | 中 | thinking 默认关 + 按任务预设 token 上限 + 缺预设拒启动；回归集用例 |
| sidecar 进程管理 | 中 | 最小子集前移 Phase 0 退出标准 |
| 视觉解析拖垮索引 Job | 中 | 视觉不进默认路径；页数/分辨率/超时配额 |
| Agent 工件格式漂移 | 低 | 采集层按普通 Markdown 解析，格式解析不进核心引擎；auto-memory 目录布局 ADR 钉住 |
| BYO 模型许可证误用 | 低 | 默认层与 BYO 层物理隔离下载清单，UI 明示 |
| mistral.rs 单维护者 | 低 | 不绑定；观察名单 |

## 十二、三模型交叉评审结论与回填记录

**评审流程**：v3.1 初稿由三个不同模型的独立评审代理在互不可见情况下评审。**结论**：A 有条件通过 7.0/10；B 有条件通过 6.4/10；C 有条件通过 6.5/10。意见收敛度高：

| 共性发现（severity 最高者） | 命中 | 处置 |
|---|---|---|
| L1.5 相位与 v3 Phase 1 硬交付冲突（抽取/打分被推迟到 Phase 2） | A major / B blocker / C major | **已回填**：Phase 1 维持 L0+L2；L1.5 Phase 2 影子运行 + 门禁切换（§四、§十） |
| 未校准置信度直接灌入 `confidence` 并进入 Fast Path 降权 | A nit / B major / C blocker | **已回填**：三层语义拆分，校准前降权关闭（§四） |
| 8GB 资源预算被三路模型+sidecar+视觉加厚，互斥策略未重写 | A major / B major / C major | **已回填**：进程级排程 + n_ctx 硬顶 + 设备矩阵重测（§二） |
| 受约束解码不在 OpenAI 兼容公共子集内；三 sidecar 行为分叉 | A major / B major / C major | **已回填**：MVP 绑定 llama.cpp server，JSON schema 契约 + fail-closed（§三） |
| `freshness` 走模型打分破坏可解释性 | C major / B 隐含 | **已回填**：freshness 保持确定性规则（§四） |
| Agent 工件读写混装、写入仓库无授权边界 | A minor / B major / C major | **已回填**：AgentArtifactSource / SkillExporter 读写分离（§五） |
| 扫描 PDF 视觉优先 = 最贵模型进默认入库路径 | A major / B major / C major | **已回填**：文本层默认，视觉为显式增强（§八） |
| 共享 OpenAI 客户端使出云边界退化为 base_url | C major（独有） | **已回填**：loopback 默认 + 三重门（§三） |
| "MRL 截维度不重建"不成立；存储维 vs 查询维 | B minor / C major | **已回填**：维度建表固定、改维=全量重建（§六） |
| 三套 L* 术语混淆（L1 未定义、"与隐私同构"） | A major / B major / C minor | **已回填**：三轴正交表（§四） |
| num_predict"必填无默认"是漏配炸弹 | B minor / C major | **已回填**：按任务预设 + 缺预设拒启动（§二） |
| Gemma 4 动因悬空（正文零提及） | B minor / C major | **已回填**：ADR③ 观察候选 + 许可证复审前提（§六） |
| "Phase 6"不存在于 v3 路线图；OCR 章节引用错；CLI 计数矛盾 | C nit/major | **已回填**：校准改"路线图外后置项"；引用更正；Adapter 承诺=三件（§五） |
| 评测池一次加五候选 → 矩阵爆炸 | B minor | **已回填**：Phase 1 只加两席，其余 BYO（§六） |
| GPUI 触发条件不可测 | B nit | **已回填**：写成可测阈值（§七） |

**未采纳/部分采纳**：A 建议任务层/算力层双术语体系——以三轴正交表简化采纳；B 建议 L1.5 下沉 Phase 1（方案 B）——采纳其方案 A（Phase 1 维持 L0+L2）；C 建议低置信升 L2 与 B"低分/schema 失败→L2"一致，合并采纳。

**ADR 清单（批次一，v3.1 版）**：③embedding 选型与许可证审查（扩充：+gte 两席、chunk 上限入池规则、Gemma 4 观察候选、jina v2-base-code 保留对照）；④摘要运行时与 L2 档位（修订：Qwen3.5 阶梯 + llama.cpp server 单后端 + loopback 出云门 + 任务级 token 预设）；**⑩L1.5 决策层（相位/门禁/级联路由/置信度三层语义/三轴正交表）**；**⑪Agent 工件读写分离与 compaction 验收**；**⑬扫描页处理策略**。GPUI 以观察记录跟踪，不占编号。**定稿前以上 ADR 与 v3 批次一 ①–⑨ 交叉索引，避免双文档编号漂移。**

---

*本文档为 Architecture Baseline v3.1（交叉评审回填后定稿）。*
