# Design — Workspace 脚手架与批次一 ADR

> 本文档只记录本 change 的落地决策。架构级"为什么"见引用的基线文档，不在本文重述。

## 1. Crate 划分与依赖方向

依据《详细设计阶段输出优先级结论》批次四与 v3.1 §五 Adapter 语义，首版 workspace 划分七个 crate：

```
mnemex-cli ──────────┐
mnemex-adapter-mcp ──┤
mnemex-adapter-http ─┼──→ mnemex-retrieval ──→ mnemex-store ──→ mnemex-core
mnemex-ingest ───────┘         （检索/排序）    （存储抽象）   （KO 领域模型）
```

| Crate | 职责边界 | 首版内容 |
|---|---|---|
| `mnemex-core` | Knowledge Object 类型、生命周期字段、事件类型；无任何 IO | 类型骨架 + 文档注释 |
| `mnemex-store` | `VectorStore`/存储 trait 抽象（v3.1 §3.1 强制要求），SQLite/LanceDB 实现留后续 change | 空 trait 占位 + 文档注释 |
| `mnemex-ingest` | 扫描/解析/分块管线（Phase 0 实现） | 空模块占位 |
| `mnemex-retrieval` | Fast/Deep Path、RRF 融合、排序（Phase 1 实现） | 空模块占位 |
| `mnemex-adapter-mcp` | dual-era MCP Adapter（Phase 2 实现） | 空模块占位 |
| `mnemex-adapter-http` | 独立本地 HTTP/JSON Adapter（Phase 2 实现） | 空模块占位 |
| `mnemex-cli` | 二进制入口：Phase 0 期的索引/校验命令载体（Tauri 到位前的主要人工入口） | `main.rs` 打印版本 + scope 说明 |

**依赖方向红线**（写进各 crate 的模块文档注释，验收时人工核对）：
- `mnemex-core` 不依赖任何内部 crate；
- adapter 之间互不依赖（v3.1 §5.3：HTTP 与 MCP 只共享 Engine 接口）；
- Agent 工件读写（Phase 1/4）未来落在 `mnemex-ingest`（读）与独立 `mnemex-exporter`（写），不在本 change 建占位。

## 2. 工具链策略

- `edition = "2024"`；`rust-version = "1.94"`（workspace 级统一声明）——取 v3.1 记录的 LanceDB MSRV，本机 rustc 1.99.1 满足；
- `[workspace.lints]`：启用 `clippy::all` 与 `warn` 级基础集，`cargo clippy --workspace` 作为验收命令之一；
- **本 change 不添加任何第三方依赖**（连 `clap`/`serde` 都不加）——依赖准入红线：默认层仅 Apache-2.0/MIT，且须过 ADR 记录（对应 ADR-003 许可证审查精神）。

## 3. ADR 结构与编号映射

ADR 文件 = `docs/adr/ADR-NNN-<slug>.md`，统一骨架：`状态 / 背景 / 决策 / 备选方案 / 后果 / 引用`。内容为 v3/v3.1 定稿结论的**转写**（不改语义），引用指向基线文档小节。

| ADR | 主题 | 来源 |
|---|---|---|
| ADR-001 | 向量库首选 LanceDB、sqlite-vec 回退与四项退出标准 | v3 §3.1 |
| ADR-002 | 双库一致性协议（SQLite 为 source of truth） | v3 §3.1 |
| ADR-003 | Embedding 选型与许可证审查（评测池、Provider 三分层、MRL） | v3 §3.2 + v3.1 §六 |
| ADR-004 | 摘要运行时与 L2 档位（Qwen3.5 阶梯、llama.cpp server 单后端、loopback 出云门、任务级 token 预设） | v3.1 §二/§三 |
| ADR-005 | Kuzu 移除与嵌入式图库观察名单 | v3 §2.2/§3.4 |
| ADR-006 | MCP dual-era 适配与 handle 语义 | v3 §五 + v3.1 §5.2 |
| ADR-007 | Fast Path RRF 融合与 freshness 规则参与排序 | v3 §2.3 |
| ADR-008 | Phase 1 回归评测集门禁定义 | v3 §4.5 |
| ADR-009 | 解析/索引层借力 vs 自研裁决（Phase 0 末出结论） | v3 §4.4 |
| ADR-010 | L1.5 结构化决策层（相位/门禁/级联路由/置信度三层语义/三轴正交表） | v3.1 §四 |
| ADR-011 | Agent 工件读写分离与 compaction 验收 | v3.1 §五 |
| ADR-012 | 扫描页处理策略（文本层默认、视觉转写显式增强） | v3.1 §八 |
| 观察记录 | GPUI 评估（维持 Tauri + 三条重评估触发条件） | v3.1 §七；**不占 ADR 编号** |

状态约定：`Proposed`（待评审）/ `Accepted`（基线已定稿，本批全部为 Accepted）/ `Superseded`（被后续 ADR 替代时标注）。

## 4. OpenSpec config 补全

`openspec/config.yaml` 填入 `context`：工件语言中文、Windows 为主开发/目标平台、MSRV 1.94、依赖许可证红线、基线文档链为 propose 输入源。避免后续每个 change 的 agent 重新推断这些约束。

## 5. 明确出界（留后续 change）

- LanceDB Spike + 一致性协议实现 → change `add-lancedb-spike`（Phase 0 末，产出=报告，分叉点留给用户）
- tree-sitter 四语言解析 → change `add-phase0-ingest-pipeline`
- KO 模型细化与事件存储 → change `add-knowledge-object-model`
- MCP dual-era 实现、Tauri UI、评测集 → 各自独立 change
