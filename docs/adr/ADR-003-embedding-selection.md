# ADR-003：embedding 选型与许可证审查

## 状态

Accepted

## 背景

默认分发的权重必须能再分发。`jina-embeddings-v2-base-code` 是 Apache-2.0；CC BY-NC 4.0 的是新系 `jina-code-embeddings`。把"jina 一类"全部判掉，会误伤仍可商用的 v2-base-code。EmbeddingGemma-2 的模型卡附带 Gemma 使用条款，且 float16 会静默产出 NaN。

## 决策

**候选矩阵（Phase 1 评测裁决默认档，基线不锁死赢家）**：

| 候选 | 许可证 | 定位 |
|---|---|---|
| Qwen3-Embedding-0.6B | Apache-2.0 | 代码与文本的默认候选 |
| Qwen3-Embedding-4B | Apache-2.0 | 有独立显存预算时的质量档 |
| jina-embeddings-v2-base-code | Apache-2.0 | 轻量对照，保留 |
| CodeRankEmbed | MIT | CPU 小型代码对照 |
| EmbeddingGemma-2（文本塔） | Apache-2.0 + Gemma 条款 | 移出纯 Apache-2.0 默认阶梯；采用前须单列条款、只加载文本塔、禁用 float16 |
| Nomic Embed Code（7B） | Apache-2.0 | 不评估 7B 权重分发 |

**Provider 三分层**（与推理能力、隐私正交，见 ADR-010）：

- 默认下载层：仅 Apache-2.0 或 MIT。
- BYO 层：用户自带，不进默认裁决。当前名单含 nomic-embed-text-v1.5，以及 jina-embeddings-v5 系（CC BY-NC，商业授权另售）。
- 云端层：显式开启，受出云门约束（ADR-004）。

**入池规则**：chunk 上限超过 512 token 的模型不入池（mxbai-embed-large-v1 因此排除）。chunk 上限在本决策写死为 512 token。

**评测池增量只加两席**：`gte-modernbert-base`、`gte-reranker-modernbert-base`。低端档默认无 reranker，Deep Path 关闭时不下载。一次加入更多候选会把回归门变成打榜。

**MRL**：

- 向量表维度在建表时固定。改维度等于全量重嵌，与换模型同级。"截维度不用重建"作废。
- `VectorStore` 区分存储维与查询维，维度写入索引元数据。
- 默认全维存储。截断只作为低端机的显式配置，不作为产品能力宣传。代码与中文检索的下限档为 256 / 512。
- BGE-M3 固定 1024 维只作对比说明，评测前不构成淘汰扣分。

**运行时分工**：embedding 走 ort（ONNX）；摘要走 llama.cpp / GGUF（ADR-004）。ort 不加载 Q4_K_M GGUF。

**Gemma 4**：观察候选。许可证复审确认附加条款撤销后，才可进入默认下载层与评测池。EmbeddingGemma-2 维持上面的 SPDX 处置。

双向量空间（文本与代码分开）降为待决问题：若单一 Qwen3-Embedding-0.6B 在中英文档与代码上同时过评测线，收敛为单空间。

## 备选方案

- 按榜单直接锁死一个默认模型：Phase 1 评测尚未跑。
- 保留"截维度不用重建"：只对"全维入库、查询时截断"成立，不能当作改存储维的策略。

## 后果

- 新的默认权重在自己的 change 里写明名称、版本、许可证、维度。
- Reranker 候选 Qwen3-Reranker-0.6B（Apache-2.0）只存在于 Deep Path；jina reranker 因 NC 风险不采用。

## 引用

- v3 §3.2
- v3 §3.3 第 2 点（ort 与 GGUF 分工）
- v3.1 §四（模型分发轴）
- v3.1 §六
