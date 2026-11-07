# Design

## Context

脚手架期 `mnemex-core` 的 ko / lifecycle / event / ids 四模块只有文档注释（见 change `add-workspace-scaffold-and-adrs` design.md）。本 change 把 ADR 与基线里的文字语义落为可编译的类型契约。约束：零第三方依赖（含 serde / 时间库 / 哈希算法库 / trybuild），生产路径无 `unwrap`/`expect`，错误走 `Result`；标识符与代码注释英文，工件中文。

## Goals / Non-Goals

**Goals:**

- 五类型 + 统一契约 + 生命周期字段 + 事件枚举 + 标识类型，全部可编译、有单元测试
- 类型层面锁死三条基线语义：`model_confidence`/`confidence` 不混用、`freshness` 无模型入口、操作只产生事件不写数值字段
- 下游 crate（store / ingest / retrieval / adapter）能直接引用这套类型开工

**Non-Goals:**

- 序列化格式（serde/JSON 形状留给 adapter change）
- SQLite 表结构与事件存储（store change）
- 哈希算法与 id 生成策略（ingest change 届时走依赖准入）
- 校准逻辑、L1.5 抽取、矛盾检测计算（后置相位）

## Decisions

### D1 组合而非继承：`KoCore` 内嵌 + `trait KnowledgeObject`

批次二落地建议是 trait 而非类继承。具体形态：公共信息收进一个 `KoCore { id: KoId, refs: SourceRefs, lifecycle: Lifecycle, created_at: UnixMillis }`，每个类型 struct 内嵌 `core: KoCore` 加自己的语义字段；`trait KnowledgeObject { fn core(&self) -> &KoCore; fn kind(&self) -> Kind; }` 由各类型实现，其余访问（id/lifecycle/refs）走 trait 默认方法。

备选：把公共字段平铺进每个 struct（重复、易漂移）；trait 里塞所有字段 getter（trait 膨胀）。内嵌 `KoCore` 让"五类型共享生命周期"在结构上可见。

### D2 关系边不内嵌进 KO 类型

v2 说 CodeSymbol "带调用/继承/import 关系"、CommitEvent "关联受影响的 Knowledge Object"。**满足方式是引用而非内嵌**：调用边存 SQLite edge 表（v3 §2.2 / ADR-005），CommitEvent 与受影响对象的关联由事件流（`Created` 事件携带 commit）与 store 查询表达。core 类型只存身份与语义字段，不存 `Vec<关系>`——内嵌会把 store 的图查询结果形态焊死进领域类型。

### D3 标识全部为不透明 newtype，长度策略按用途分

| 类型 | 形态 | 理由 |
|---|---|---|
| `KoId` | `[u8; 16]` | 固定长、拷贝便宜；生成策略留给 ingest |
| `ChunkId` | `[u8; 16]` | 同上；与 `KoId` 是不同类型，ADR-002 对齐键不混用 |
| `ContentHash` | `Box<[u8]>`（可变长，非空校验） | 算法未定（ADR-003 由 ingest 裁决），固定长度会预支承诺 |
| `CommitSha` | `Box<[u8]>`（可变长） | 兼容 git sha-1(20B)/sha-256(32B) 过渡 |

备选：全部 `String` 十六进制（浪费一次编码且仍需防混用 newtype）；全部定长（sha-256 过渡会破坏 `CommitSha`）。spec 场景"标识防混用"由 newtype 结构本身保证——不同 `struct` 无隐式转换，不需要 trybuild 编译失败测试（那要引第三方依赖）。

### D4 置信度双字段用两个不同 newtype，均为 `Option`

`ModelConfidence(f32)` 与 `Confidence(f32)` 是不同 struct，构造函数校验 `[0,1]` 区间并返回 `Result`（错误类型手写 enum 实现 `std::error::Error`，不引 thiserror）。`Lifecycle` 中两字段均 `Option`：`model_confidence` 无模型抽取时缺省；`confidence` 在校准落地前恒 `None`——"未校准"徽标的数据来源。备选：单一 `confidence: f32`（被 v3.1 §四明确否决）。

### D5 `freshness` 为规则来源专用形态

`freshness: Freshness`，`enum Freshness { Fresh, Stale }`（确定性规则产物：git/hash/mtime，ADR-007）。类型上不存在"模型给 freshness 打分"的构造入口。用户标记过时是事件（`MarkedStale`）+ 用户来源标记，不落 freshness 字段——展示层合并双来源（原型设计 v1"规则判定 · 你标记为"语义）。备选：分值形式（预支了不存在的评分模型）。

### D6 事件：私有字段 + 访问器，保证不可变

`enum KnowledgeEvent` 变体：`Created / Used / Verified / MarkedStale / MarkedContradictory / Archived / Feedback { useful }`。每变体携带 `ko: KoId`、`at: UnixMillis`、`source: EventSource`，`enum EventSource { Rule, Model, User }`（展示层区分规则/模型/用户触发，原型设计时间线语义）。字段一律私有，构造走关联函数、读取走访问器方法——"不可变"在编译期成立，不靠约定。备选：pub 字段 + 文档约定（`&mut` 即可破坏，过不了 spec 场景"事件不可变"）。

> **落地修正（apply 阶段回填）**：Rust 的 enum 变体字段在语言层面不可能私有。实际落地为 struct `KnowledgeEvent`（私有字段：kind/ko/at/source）包裹 `EventKind` 枚举（七变体，`Feedback` 携 `useful`）——每个操作一个关联构造函数，访问器返回 Copy 值，编译期不可变契约与原设计一致。

### D7 时间用 `UnixMillis(i64)` newtype

`created_at` / `last_used` / 事件时间戳统一 `UnixMillis(i64)`（UTC 毫秒）。不引 chrono/time：本阶段无格式化与时区需求，展示层格式化是 adapter/UI 的事。备选：`std::time::SystemTime`（不可比较语义弱、无序列化友好形态）。

### D8 五类型的语义字段（首版草案）

| 类型 | 特有字段（首版） | 基线依据 |
|---|---|---|
| `CodeSymbol` | `name`、`symbol_kind`（function/class/module）、`container`（所属模块路径，`Option`） | v2 §二 |
| `DesignDecision` | `problem`、`diagnosis`、`resolution`（三元组） | v2 §二；原型 v1 详情页三卡 |
| `BugRecord` | `symptom`、`root_cause`、`fix` | v2 §二 |
| `DocumentSection` | `title`、`heading_path`（章节层级路径） | v2 §二 |
| `CommitEvent` | `sha: CommitSha`、`message`、`authored_at` | v2 §二（受影响对象关联见 D2） |

字段全为 `String`（UTF-8 语义边界即够；借用语义在 store/adapter 层再议）。字段集在 store 表结构 change 前仍可微调，但 D1–D7 的结构语义先锁。

## Risks / Trade-offs

- [无 serde，后续序列化可能要求调整字段可见性] → 字段语义以稳定为先；序列化形状由 adapter change 单独定，不在本 change 预支。
- [`Option<Confidence>` 恒 `None` 显得冗余] → 有意为之：类型先于校准存在，"未校准"是 UI 语义（ADR-007 降权关闭）。
- [私有字段 + 访问器样板多] → 换取编译期不可变（spec 硬场景）；`#[derive(Clone, Debug, PartialEq, Eq)]` 控制成本。
- [`CommitEvent` 不内嵌受影响对象列表] → 与 v2 表述的差异在 D2 记录；若 store change 证明需要内嵌，届时按证据改。
- [`ContentHash` 无长度上限] → 构造时非空校验；算法引入时（ingest change）再收紧。

## Migration Plan

纯新增：四个模块由注释骨架变为具体类型，无既有行为可破坏。回滚 = revert 单个 change 的 commit。验收命令组与 rust-workspace spec（构建健康）一致。

## Open Questions

- `KoId` 的 16 字节生成策略（随机 / 内容寻址 / ULID 类）——ingest change 决定，不影响本 change 类型形状。
- `DocumentSection.heading_path` 的分隔约定（`/` 级联还是段落数组）——store 表结构定稿时顺手定，`Vec<String>` 还是拼接字符串两可，当前按拼接字符串占位。
