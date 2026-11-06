# ADR-010：L1.5 结构化决策层

## 状态

Accepted

## 背景

流水线里的结构化判定卡在 L0（规则脆）和 L2（生成重）之间。这一层若在 Phase 1 就写知识对象，会和"记忆对象 MVP 不依赖未验证层"冲突。能力、分发、隐私是三件事，不能编成一个 L 编号。

## 决策

**三轴正交**：

| 轴 | 级别 |
|---|---|
| 推理能力 | L0 规则 / L1 embedding（ort）/ L1.5 受约束解码 / L2 生成 / L3 云端 |
| 模型分发 | 默认下载层（Apache-2.0 或 MIT）/ BYO / 云端（显式开启） |
| 隐私 | 目录级"禁止出云"，与上两轴正交 |

"Provider 三分层与隐私 L0–L3 同构"的说法作废。

**相位**：

| 阶段 | L1.5 |
|---|---|
| Phase 1 | 不存在。抽取与打分维持 L0 + L2 |
| Phase 2 上线 | chunk 类型分类 + 影子运行（只记日志，不挡入库，不写库）。门禁：决策抽取质量不低于纯 L2，且 8GB 上 chunk 分类延迟达标 |
| 门禁通过后 | DesignDecision 三元组预筛、BugRecord 识别、提醒价值评分才切到 L1.5。切换前 DesignDecision 的落库者始终是 L2 |
| 路线图外 | 置信度校准、LoRA。不存在"Phase 6" |

**置信度三层**：JSON 合法（`schema_valid`）≠ 原始分 ≠ 校准后的 `confidence`。知识对象字段拆成 `model_confidence`（原始）与 `confidence`（校准后）。校准前，Fast Path 的 confidence 降权关闭（ADR-007）。

**级联**：L0 能判定的不调模型 → L1.5 出候选 → 低分或 schema 失败升 L2。校准前的高分不得单独落库。初稿"用 L2 复核高置信样本"的方向作废。

`contradiction_score` 只对近邻候选（同符号、同文件、向量近邻）调用 L1.5，并单列预算。

Jev AI 不进默认链路。它留在 L3 云端增强的观察名单，命中"禁止出云"则硬拦。

## 备选方案

- Phase 1 就让 L1.5 写 DesignDecision：切换门禁还不存在。
- 把本地 / 云编进 L 编号：换分发方式会改掉能力语义。

## 后果

- 受约束解码不在通用 OpenAI 兼容子集里，因此 MVP sidecar 只绑定 llama.cpp server（ADR-004）。
- L1.5 的实现 change 必须带影子模式，默认不写库。

## 引用

- v3.1 §四
- v3.1 §三（`DecisionProvider` 失败回退 L0）
- v3.1 §十二（freshness、L1.5 相位、出云门的回填记录）
