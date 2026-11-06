# 添加 Rust Workspace 脚手架与批次一 ADR

## Why

Mnemex 的架构基线（v2 → v3 → v3.1）已定稿并通过三轮三模型交叉评审，进入工程实现阶段。按《详细设计阶段输出优先级结论》，**批次一（决策先行：ADR）与批次四的 Workspace 脚手架**是所有后续详细设计与实现的前置件：

- v3.1 §十一明确要求"评审闭合后再开 ADR，ADR 按回填后的结论写"——评审已闭合（2026-11-05），ADR 现在应立项，防止已拍板的决策（LanceDB 首选+回退、dual-era MCP、L1.5 相位等）在实现阶段被重新争论或悄悄漂移；
- 后续每个 OpenSpec change（Phase 0 扫描器、LanceDB Spike、KO 模型、检索、适配器、UI）都需要一个 `cargo build`/`cargo test` 全绿的 workspace 作为落点与验收载体；
- 仓库目前只有文档（`docs/brainstorming/`）与 OpenSpec 骨架，没有任何 Rust 代码。

本 change 是 goal 链的**第 0 项**：纯机械性、验收可完全机器化、无分叉点，适合一次性执行。

## What Changes

1. **新增 `docs/adr/`**：12 条决策 ADR（编号 ADR-001…ADR-012）+ 1 份 GPUI 观察记录（不占 ADR 编号，依 v3.1 §十一）+ 索引 README。内容全部转写自 v3/v3.1 已定稿结论，不引入新决策；每条含背景/决策/备选方案/后果/基线引用。
2. **新增 Rust workspace 脚手架**：根 `Cargo.toml`（edition 2024、MSRV 1.94、统一 lint 配置）+ 七个 crate 骨架（`mnemex-core`、`mnemex-ingest`、`mnemex-store`、`mnemex-retrieval`、`mnemex-adapter-mcp`、`mnemex-adapter-http`、`mnemex-cli`），依赖方向按《详细设计优先级结论》批次四：外层 crate 依赖 core，core 不依赖任何内部 crate，adapter 之间互不依赖。
3. **补全 `openspec/config.yaml` 的 project context**：把 agent 无法从代码推断的项目约束（中文工件、Windows 优先、MSRV 1.94、脚手架期依赖准入红线）写入 context。

**本 change 不做**（明确出界，留后续 change）：LanceDB/树解析等任何第三方依赖引入；任何业务逻辑代码（除占位模块与文档注释）；MCP 协议实现；Tauri UI；评测集。

## Impact

- Affected specs（均为新建，无既有 spec 被修改）：
  - `rust-workspace`：ADDED（构建健康、依赖方向、MSRV 声明、依赖准入红线）
  - `decision-records`：ADDED（ADR 覆盖面、结构完整性、与基线的一致性、状态标记）
- 参考输入（只读引用，不修改）：`docs/brainstorming/Mnemex藏识-架构与路线-v3-2026开源趋势修订.md`、`…-v3.1-补充修订.md`、`…-详细设计阶段输出优先级结论.md`
- 风险：低。纯新增文件，无行为变更；唯一注意点是 MSRV 声明需与本机工具链（1.99.1）及 LanceDB MSRV（1.94）一致。
