# Proposal

## Why

SQLite 模式层已经能记下 chunk 对齐键和待索引位，但 `VectorStore` 仍是空 trait，ADR-002 的补偿流没有可运行的第二段。ADR-001 把 LanceDB 定为 Phase 0 首选，并要求用桌面场景的四项退出标准验证，而不是凭上游活跃度冻结选型（v3 §3.1）。

## What Changes

- 准入 `lancedb` 0.41.0（2026-10-29 发布，未 yanked；Apache-2.0；MSRV 1.94.0，与 workspace 一致）。近两月有 0.38 至 0.41 的稳定发布。未发现指向 `lancedb` crate 本身的 RUSTSEC；上游 `deny.toml` 仍忽略若干传递依赖公告（rsa、instant、paste、tantivy、quick-xml）。Python 0.41 的远程代码门禁不在本仓库使用的 Rust 路径上。
- 给 `VectorStore` 补上按 `chunk_id` + `content_hash` 写入、按该对齐键精确删除、以及从 SQLite 全量重建。向量由调用方传入，本 change 不跑嵌入模型。
- 落地补偿流的可运行切片：SQLite chunk 行先提交且 `pending=1`；向量写入成功后才清位；两步之间失败则保持待索引；重建能从 SQLite 把缺的向量补上。哈希校验只核对对齐键，不引入跨库事务（ADR-002）。
- ADR-001 四项退出标准记成 spike 观察，不做成产品阈值，也不编造通过数字。自动化测试覆盖小规模合成数据上的补偿；10 万级重建、Tauri 打包与 Defender 能跑则记实测，跑不了则记阻塞原因。

## Capabilities

### New Capabilities

- `vector-store`: 向量索引作为 SQLite 的派生存储：对齐键写入与删除、全量重建、以及 pending 位上的补偿顺序。

### Modified Capabilities

- `sqlite-infra`: 「第三方依赖最小面」从仅 rusqlite 一系，扩大为 rusqlite 与 lancedb 两系。其余库级参数不变。

## Impact

- `mnemex-store`：`VectorStore` 方法、LanceDB 本地实现、与现有 `insert_chunk` / `set_chunk_pending` 衔接的补偿入口。
- 依赖：直接第三方从 rusqlite 增加 lancedb 0.41.0 及其传递闭包。不引入 tempfile、thiserror，也不实现 sqlite-vec。
- 不改知识对象类型、四表 DDL、FTS5 内容表、摄入、边查询、sidecar 与 UI。
