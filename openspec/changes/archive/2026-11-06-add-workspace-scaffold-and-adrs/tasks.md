# Tasks — Workspace 脚手架与批次一 ADR

## 1. ADR 基础设施（批次一：决策先行）

- [x] 1.1 创建 `docs/adr/README.md` 索引：编号、主题、状态、基线引用对照表（12 ADR + 1 观察记录）
- [x] 1.2 写 ADR-001（向量库首选/回退/四项退出标准）——转写 v3 §3.1，含 sqlite-vec 回退条件
- [x] 1.3 写 ADR-002（双库一致性协议：SQLite source of truth、chunk_id+content_hash、三段补偿流）
- [x] 1.4 写 ADR-003（embedding 选型与许可证审查：候选矩阵、Provider 三分层、MRL 政策、chunk 上限入池规则）
- [x] 1.5 写 ADR-004（摘要运行时与 L2 档位：Qwen3.5 阶梯、llama.cpp server 单后端、loopback 出云门、任务级 token 预设、thinking 关闭）
- [x] 1.6 写 ADR-005（Kuzu 移除；观察名单 LadybugDB/Grafeo/Lance Graph 及独立 ADR 准入）
- [x] 1.7 写 ADR-006（MCP dual-era：2026-07-28 主路径 + initialize 兼容；client-supplied scope 与 server-minted handle 分工；resultType/MRTR）
- [x] 1.8 写 ADR-007（Fast Path RRF：FTS5 与向量各取 top-k、RRF 融合、freshness 规则降权、confidence 降权校准前关闭）
- [x] 1.9 写 ADR-008（Phase 1 回归评测集门禁：三类任务 ≥15 条、回归定位、与竞品对比留 Phase 2）
- [x] 1.10 写 ADR-009（借力 vs 自研决策门：Phase 0 末二选一，输出一页结论，阻抗在数据模型）
- [x] 1.11 写 ADR-010（L1.5：Phase 2 影子运行、切换门禁、级联路由 L0→L1.5→L2、置信度三层语义、三轴正交表）
- [x] 1.12 写 ADR-011（Agent 工件读写分离：AgentArtifactSource 只读 / SkillExporter 显式授权；compaction 两条验收）
- [x] 1.13 写 ADR-012（扫描页：默认文本层、扫描页标记跳过、视觉转写 Phase 2+ 显式增强与配额）
- [x] 1.14 写 GPUI 观察记录 `docs/adr/OBSERVATION-gpui.md`（不占编号；三条可测触发条件）
- [x] 1.15 逐条核对：每份 ADR 均含"状态/背景/决策/备选/后果/引用"六段，且引用指向 v3/v3.1 具体小节

## 2. Rust Workspace 脚手架

- [x] 2.1 创建根 `Cargo.toml`：`workspace.members`（7 crate）、`edition = "2024"`、`rust-version = "1.94"`、`[workspace.lints]`（clippy::all + warn 基础集）、`[workspace.package]` 统一元数据
- [x] 2.2 `mnemex-core`：`lib.rs` 模块骨架（`ko`、`lifecycle`、`event`、`ids`）+ 文档注释（KO 五类型、生命周期五字段、事件溯源语义），**无第三方依赖**
- [x] 2.3 `mnemex-store`：`VectorStore` trait 空占位 + 文档注释（ADR-002 一致性协议、可重建语义），依赖仅 `mnemex-core`
- [x] 2.4 `mnemex-ingest`：空模块占位（scan/parse/chunk），依赖仅 `mnemex-core`
- [x] 2.5 `mnemex-retrieval`：空模块占位（fast/deep、fusion、rank），依赖 `mnemex-core` + `mnemex-store`
- [x] 2.6 `mnemex-adapter-mcp` 与 `mnemex-adapter-http`：空模块占位，依赖仅 core 侧接口，**互不依赖**
- [x] 2.7 `mnemex-cli`：`main.rs` 打印版本号 + scope 概念说明（无第三方依赖，std 参数解析）
- [x] 2.8 各 crate 的 `lib.rs` 顶部写入依赖方向红线注释（core 零内部依赖；adapter 互不依赖）

## 3. OpenSpec 配置补全

- [x] 3.1 在 `openspec/config.yaml` 填入 `context`：工件语言中文；Windows 主开发/目标平台；MSRV 1.94；依赖准入（默认层 Apache-2.0/MIT only）；propose 输入源 = `docs/brainstorming/` 基线文档链

## 4. 验收（全部命令化）

- [x] 4.1 `cargo build --workspace` 退出码 0
- [x] 4.2 `cargo test --workspace` 退出码 0（骨架期允许 0 个测试，但命令必须绿）
- [x] 4.3 `cargo clippy --workspace` 无 error（warn 基线内）
- [x] 4.4 依赖方向核对：`cargo tree -i mnemex-core` 输出无反向边；`mnemex-adapter-mcp` 与 `mnemex-adapter-http` 的依赖清单互不包含
- [x] 4.5 `openspec validate add-workspace-scaffold-and-adrs --strict` 通过
- [x] 4.6 `docs/adr/README.md` 索引条目与实际文件一一对应（12 ADR + 1 观察记录）
