# Mnemex / 藏识 — Agent 工作宪法

本地工程记忆引擎（AI Agent 的 Local Engineering Memory Engine）：Rust + Tauri 2 桌面应用，扫描本地文档与代码构建 Knowledge Object 知识库，经 dual-era MCP / HTTP Adapter 对 Agent 开放。隐私优先、Windows 为主开发与目标平台。当前处于工程启动期（首个 change：workspace 脚手架 + 批次一 ADR）。

## 真相源优先级

冲突时以高层为准；低层是历史记录：

1. `openspec/specs/`（系统今日行为，逐 change 累积）
2. `docs/adr/`（架构决策记录；首个 change 落地后生效）
3. `docs/brainstorming/Mnemex藏识-架构与路线-v3.1-补充修订.md`（最新架构基线；ADR 未建时为准）
4. 同目录其余 v1/v2/v3 文档与 `藏识Mnemex-UI设计规范.md`、`原型设计-v1.1`（历史/领域背景）

`docs/brainstorming/` 是**只读历史**：任何 change 不得修改它；演进一律走 OpenSpec delta。语义拿不准时对照 v3.1 第十二节"评审回填记录"——那里的表述是评审否决初稿后的定稿语义（如 freshness 走确定性规则、出云门是 denylist、L1.5 有相位门禁），不是任何早期文档的说法。

## 工作流（OpenSpec，强制顺序）

1. **任何变更/新功能/修复，动手前先 `/opsx:explore`**：读基线对应小节 + 相关代码，产出探索结论（只读，不写文件）。
2. `/opsx:propose` 创建 change（proposal / design / tasks / spec delta），引用基线小节编号而非复述。
3. **停在人工闸门**：propose 完成后等用户审阅通过；重大分叉点（选型、相位推进）即使有把握也必须请示。
4. `/opsx:apply` 按/tasks.md 顺序执行（TDD，见下）。
5. 每项验收前跑 `openspec validate <change> --strict`；归档用 `openspec archive`（不手改 specs/）。
6. 一次 change 只做一个里程碑；Phase 3/4 相关工作在 usage_count 真实数据积累前**不开工**（v3.1 数据闸门）。

## TDD 纪律

红 → 绿 → 重构。写实现前先写会失败的测试；任务勾选的前提是**该任务的三层测试全部绿**，且 `cargo test --workspace` 绿。修 bug 先写复现该 bug 的红测试。

三层测试的边界：

| 层 | 位置 | 要求 |
|---|---|---|
| 单元 | 各 crate `#[cfg(test)]`，针对单模块 | 毫秒级；无 IO、无睡眠 |
| 集成 | crate 间（如 store↔core 一致性协议、ingest→retrieval 全管线） | 用真实 SQLite 与临时目录，**不 mock 文件系统** |
| 端到端 | `mnemex-cli` 对真实仓库执行全量索引+检索 | 覆盖回归评测集三类任务（ADR-008 定义）；Phase 2 起加 dual-era MCP 真实客户端冒烟 |

评测集是**回归门禁**（改分块/排序不得回退），不是打榜工具；阈值类数字（如 usage_count 解锁目标）只展示真实积累，不得虚构分母。

## 依赖与版本

- 新增或升级依赖：选当时**最新稳定版**（semver 稳定位）；pre-release / nightly / yanked 版本一律不用，例外须记 ADR。
- 引入依赖必须走独立 OpenSpec change，proposal 里写明名称/版本/**许可证**；许可证红线：默认层 Apache-2.0 / MIT only（BYO 概念只适用于最终用户侧，不适用于本仓库代码依赖）。
- 引入前检查上游健康度（近期提交、release 节奏、security advisory）——sqlite-vec 与 Kuzu 的教训已写进 v3.1 风险表。
- MSRV = 1.94（workspace 统一声明，LanceDB 约束），是**下限**不封顶；edition 2024。

## 代码约束

- 生产路径禁止 `unwrap`/`expect`；错误走 `Result` 逐层传播，错误类型经 ADR 定（依赖解锁前用 std）。
- `unsafe` 仅限 bindgen/FFI 模块，且需 ADR 登记（未来 llama.cpp FFI 那条路径）。
- crate 依赖方向：`mnemex-core` 零内部依赖；`adapter-mcp` 与 `adapter-http` 互不依赖（两协议只共享引擎接口）。
- 标识符与代码注释用英文；文档、UI 文案、提交信息用中文。
- 提交原子化，Conventional Commits（`feat:`/`fix:`/`docs:`/`chore:`）。
- 触碰 UI 时先读 `docs/brainstorming/Mnemex藏识-原型设计-v1.1.md` 与 `docs/brainstorming/demo/app.html`：置信度只显示"未校准"徽标（不显示百分比/档位）、排序摘要统一文案、coral 仅用于硬边界——这些是评审定稿语义。

## 隐私与安全红线（硬约束，无例外）

- 推理客户端默认仅 loopback；任何非 loopback 出网需三重门（L3 总开关 + 未命中禁止出云列表 + 用户单次确认），**确认后不可撤回**。
- "禁止出云"是 denylist 语义：**命中即拦截**，没有"白名单放行"这个概念。
- 数据出网、写入用户仓库（SkillExporter）、扫描页视觉转写：全部默认关闭、显式触发。
- 测试与示例中不得出现真实路径下的敏感内容。

## 完成定义

任何 change 归档前，以下全部为真：`cargo build --workspace` 绿；`cargo test --workspace` 绿；`cargo clippy --workspace` 无 error；`openspec validate --all --strict` 通过；tasks.md 全勾；新增行为有对应 spec delta 与三层测试覆盖。
