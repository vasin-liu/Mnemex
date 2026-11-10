# Design

## Context

`VectorStore` 仍是空 trait。SQLite 已有 chunk 行（`chunk_id`、`content_hash`、默认 `pending=1`）以及 `set_chunk_pending`。动机见 proposal.md。约束：零新错误库、生产路径无 `unwrap`/`expect`、向量库可替换（ADR-001 第 3 点）。

## Goals / Non-Goals

**Goals:**

- 本地 LanceDB 实现上述 trait，测试用真实数据集目录，不 mock 文件系统。
- 一条补偿入口把「SQLite 先提交、向量成功后清位、失败保持待索引、重建能补洞」跑通。
- 四项退出标准落成观察记录，而不是 CI 里的数字门禁。

**Non-Goals:**

- 嵌入、摄入、FTS 内容表、sqlite-vec 实现、边查询、sidecar、UI。
- 把 10 万级重建或 Tauri 打包写成必须在 CI 里变绿的测试。

## Decisions

### D1 依赖钉在 lancedb 0.41.0

直接依赖 `lancedb = 0.41.0`，Apache-2.0。传递闭包算这一系，不再单独准入。实现时先读该版本的 feature 列表：remote / cloud 一类能关则关，只留打开本地数据集所需的 feature。具体名单以 crate 的 Cargo.toml 为准，写进依赖注释。备选：把版本写成浮动 `0.41`——拒绝，准入要可复现。

### D2 实现藏在 trait 后面，数据与 SQLite 同目录

`VectorStore` 增加写入、按对齐键删除、按对齐键读取、全量重建。LanceDB 类型不出现在 trait 签名上。数据集目录与 SQLite 文件同在一个父目录，这样「单目录备份」才有东西可观察。备选：trait 方法直接返回 LanceDB 的表类型——会把调用边界焊死，否决。

### D3 向量由调用方传入，维度在打开时固定

写入的向量是 `f32` 切片。打开时记下维度；长度不符返回错误，不原地改维（ADR-003：改维等于全量重嵌，重嵌不在本 change）。测试用小维度（4）的合成向量。备选：把向量存进 SQLite BLOB 以便无调用方也能重建——那是第二份真相，否决。

### D4 补偿是顺序，不是跨库事务

入口顺序：chunk 行已在 SQLite 且 `pending=1` → 向量写入 → 按对齐键读回并核对 → `set_chunk_pending(false)`。任一步失败就停，不清位。重建：丢掉向量库中不在当前 SQLite chunk 行里的键，为仍待索引的行写入调用方提供的向量，成功后再清位。备选：两阶段提交——ADR-002 已否。

### D5 错误仍走手写 StoreError

新增一个向量失败变体，携带上下文文本。不引入 thiserror。LanceDB 的错误放进 `source`。未知或半截状态不另造「猜测成功」。

### D6 退出标准写在 change 内的观察笔记

`spike-notes.md` 放在本 change 目录，四项各一行：文件增删改名与分支切换下的一致性、Windows 打包 / Defender / 文件锁、10 万级增量重建的停顿、单目录备份。跑过的写实测，没跑的写阻塞原因。禁止把未实测写成通过。自动化测试只覆盖小规模补偿。

## Risks / Trade-offs

- [lancedb 传递树很大，且上游仍忽略若干传递公告] → proposal 已记下；本 change 不扩大直接依赖。cloud feature 能关则关。
- [没有嵌入就无法从原文重建向量] → 重建契约要求调用方供向量。真正的重嵌留给摄入与模型 change。
- [10 万级与 Tauri 打包在当前仓库还跑不起来] → 观察笔记记阻塞，不伪造退出标准通过。Spike 不过时的回退仍是 sqlite-vec，那个实现不在本 change。
- [版本 churn] → 钉死 0.41.0。升级另开 change。

## Migration Plan

没有已发布的用户库。回滚是 revert 本 change。向量目录可以整份删除，再从 SQLite 重建。

## Open Questions

（无。feature 的具体开关名在实现时对照 0.41.0 的清单填写，不改变契约。）
