# Tasks

## 1. ids 模块（TDD）

- [x] 1.1 红测试：为 `KoId` / `ChunkId` / `ContentHash` / `CommitSha` 写构造与校验用例（`ContentHash` 拒绝空字节、四类型可 Debug/Clone/Eq），运行 `cargo test -p mnemex-core` 确认编译失败（红）
- [x] 1.2 绿实现：四个不透明 newtype（D3 形态：id 定长 16 字节、hash/sha 可变 `Box<[u8]>` 非空校验）+ `SourceRefs`（id/content_hash/commit 为主、path 为辅），验证 `cargo test -p mnemex-core` 通过
- [x] 1.3 模块文档：把 `ids.rs` 的占位注释替换为英文 doc（引用语义：ADR-002 对齐键、ADR-011 主辅引用），验证 `cargo doc -p mnemex-core --no-deps` 退出码 0

## 2. lifecycle 模块（TDD）

- [x] 2.1 红测试：`ModelConfidence` / `Confidence` 越界值（<0、>1）构造返回 `Err`、合法值构造成功；`Lifecycle` 默认形态断言（五字段全缺省/零值、`confidence` 为 `None`）；确认测试失败（红）
- [x] 2.2 绿实现：`ModelConfidence` 与 `Confidence` 两个独立 newtype（`Result` 构造 + 手写错误 enum 实现 `std::error::Error`）、`Freshness`（Fresh/Stale 枚举，无模型入口）、`ContradictionScore`、`UnixMillis(i64)`、`Lifecycle` 结构（D4/D5/D7 形态），验证 `cargo test -p mnemex-core` 通过
- [x] 2.3 模块文档：替换 `lifecycle.rs` 占位注释为英文 doc（五字段语义、model_confidence/confidence 拆分引 ADR-010、freshness 确定性引 ADR-007），验证 `cargo doc -p mnemex-core --no-deps` 退出码 0

## 3. ko 模块（TDD）

- [x] 3.1 红测试：五种 KO 类型构造后经 `trait KnowledgeObject` 统一访问 `core()`/`kind()`；`Kind` 枚举恰好五变体；`SourceRefs` 主辅字段可缺省；确认测试失败（红）
- [x] 3.2 绿实现：`KoCore`（id + refs + lifecycle + created_at）、五类型 struct（D8 字段集：CodeSymbol/DesignDecision/BugRecord/DocumentSection/CommitEvent）、`trait KnowledgeObject`（`core()` + `kind()` + 默认访问方法）、关系边不内嵌（D2），验证 `cargo test -p mnemex-core` 通过
- [x] 3.3 模块文档：替换 `ko.rs` 占位注释为英文 doc（五类型语义引 v2 §二、统一契约），验证 `cargo doc -p mnemex-core --no-deps` 退出码 0

## 4. event 模块（TDD）

- [x] 4.1 红测试：七种操作事件（Created/Used/Verified/MarkedStale/MarkedContradictory/Archived/Feedback）构造与访问器读取 ko/at/source；`EventSource` 三态（Rule/Model/User）可辨；确认测试失败（红）
- [x] 4.2 绿实现：`KnowledgeEvent` 私有字段 + 关联构造函数 + 访问器（D6 不可变形态；Rust enum 变体字段不可私有，落地为 struct 包裹 `EventKind` 枚举，语义不变）、`EventSource` 枚举；确认事件无 mutator 方法（spec"事件不可变"场景），验证 `cargo test -p mnemex-core` 通过
- [x] 4.3 模块文档：替换 `event.rs` 占位注释为英文 doc（操作即事件、来源三态引原型设计时间线语义），验证 `cargo doc -p mnemex-core --no-deps` 退出码 0

## 5. 验收

- [x] 5.1 `cargo build --workspace` 退出码 0
- [x] 5.2 `cargo test --workspace` 退出码 0，且 `mnemex-core` 测试数 > 0
- [x] 5.3 `cargo clippy --workspace` 无 error（warn 基线内）
- [x] 5.4 `cargo tree -p mnemex-core` 输出无第三方依赖（零依赖红线）
- [x] 5.5 `openspec validate add-knowledge-object-model --strict` 通过
- [x] 5.6 场景覆盖核对：spec 四需求（五类型/生命周期/事件溯源/引用语义）的每个场景均能指到至少一个对应单测，核对表落在本文件末尾追加的小节

## 场景覆盖核对表（5.6）

实测：`mnemex-core` 单元测试 18 个，全部通过（`cargo test --workspace` 绿，clippy 无诊断，`cargo tree -p mnemex-core` 无第三方依赖）。

| spec 场景 | 对应单测 |
|---|---|
| 统一契约访问 | `ko::tests::five_types_share_one_contract`（五类型装箱为 `dyn KnowledgeObject` 走同一访问路径） |
| 类型判别完备 | `ko::tests::kind_covers_exactly_five_types`（无通配的穷尽 match，第六变体编译期破坏测试） |
| 置信度双字段分离 | `lifecycle::tests::raw_and_calibrated_confidence_are_distinct_types` |
| 校准前缺省 | `lifecycle::tests::lifecycle_default_is_uncalibrated_and_unused` + `ko::tests::five_types_share_one_contract`（trait 访问断言 confidence 为 None） |
| freshness 仅规则来源 | `lifecycle::tests::freshness_has_exactly_two_rule_variants`（穷尽 match 钉死无载荷双变体形态） |
| 操作即事件 | `event::tests::every_operation_is_an_event` + `event::tests::feedback_carries_usefulness` |
| 事件不可变 | `event::tests::events_are_immutable_after_construction`（私有字段、无 mutator 的编译期形态） |
| 事件来源可辨 | `event::tests::event_sources_are_distinguishable` |
| 标识防混用 | `ids::tests::four_id_types_are_distinct_and_derived`（newtype 结构本身保证编译期拒绝混用） |
| 主辅引用结构 | `ids::tests::source_refs_defaults_to_all_absent` + `ids::tests::source_refs_path_only_is_valid` |

## Workflow follow-up

- 人工审阅四份工件后执行 `/opsx-apply`。
- apply 完成、验收全绿后归档：`openspec archive add-knowledge-object-model --yes`，并补主 spec 的 Purpose（若 CLI 写占位）。
