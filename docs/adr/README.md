# Mnemex 决策记录索引

状态一律 **Accepted**（GPUI 观察记录为"观察"）。正文只转写基线已定稿的语义，不引入新决策，不出现评审已否决的表述。
基线：`docs/brainstorming/Mnemex藏识-架构与路线-v3-2026开源趋势修订.md`（下称 v3）、`docs/brainstorming/Mnemex藏识-架构与路线-v3.1-补充修订.md`（下称 v3.1）。引用沿用两文档各自的编号习惯：v3 大节用中文数字（§二、§三…），小节用阿拉伯数字（§3.1…）；v3.1 全部用中文数字（§二、§四…）。

| 编号 | 主题 | 状态 | 基线 |
|---|---|---|---|
| [ADR-001](./ADR-001-vector-store.md) | 向量库首选、回退与四项退出标准 | Accepted | v3 §3.1、§2.2 |
| [ADR-002](./ADR-002-dual-store-consistency.md) | 双库一致性协议 | Accepted | v3 §3.1 |
| [ADR-003](./ADR-003-embedding-selection.md) | embedding 选型与许可证审查 | Accepted | v3 §3.2、§3.3；v3.1 §六 |
| [ADR-004](./ADR-004-summary-runtime.md) | 摘要运行时与 L2 档位 | Accepted | v3 §3.3；v3.1 §二、§三 |
| [ADR-005](./ADR-005-kuzu-removed.md) | Kuzu 移除 | Accepted | v3 §2.2、§3.4 |
| [ADR-006](./ADR-006-mcp-dual-era.md) | MCP dual-era 与 scope | Accepted | v3 §2.1、§5.1–5.3 |
| [ADR-007](./ADR-007-fast-path-rrf.md) | Fast Path RRF 融合 | Accepted | v3 §2.3、§3.1；v3.1 §四 |
| [ADR-008](./ADR-008-eval-gate.md) | Phase 1 回归评测集门禁 | Accepted | v3 §4.5、§4.1 |
| [ADR-009](./ADR-009-build-vs-buy.md) | 借力 vs 自研决策门 | Accepted | v3 §4.4 |
| [ADR-010](./ADR-010-l15-structured-decision.md) | L1.5 结构化决策层 | Accepted | v3.1 §四、§三 |
| [ADR-011](./ADR-011-agent-artifacts.md) | Agent 工件读写分离 | Accepted | v3.1 §五 |
| [ADR-012](./ADR-012-scan-pages.md) | 扫描页策略 | Accepted | v3.1 §八 |
| [OBSERVATION-gpui](./OBSERVATION-gpui.md) | GPUI 观察记录（不占编号） | 观察 | v3.1 §七 |
