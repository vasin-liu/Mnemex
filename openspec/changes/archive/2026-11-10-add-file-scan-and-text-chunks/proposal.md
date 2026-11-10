# Proposal

## Why

SQLite 已能保存知识对象与 chunk 对齐键，LanceDB 补偿流也能在调用方提供向量时追平，但 Phase 0 的文件扫描还没有写入者（v3 §七）。`chunks` 只有对齐键、没有正文，FTS5 只在打开库时做冒烟，Fast Path 没有字可搜，以后重嵌也没有正文可取。

## What Changes

- 准入 `sha2` 0.11.1（2026-10-12 发布，未 yanked；MIT OR Apache-2.0；MSRV 1.88，低于 workspace 1.94）。直接依赖加在 `mnemex-ingest`。`digest` 是这一系的传递依赖，不单独准入。内容哈希用 SHA-256，补上知识对象模型留给摄取阶段的算法选择。
- 新增文本摄取：递归扫描目录，跳过固定拒绝名单（`.git`、`target`、`node_modules`）与非 UTF-8；只收 `.md`、`.markdown`、`.txt`。Markdown 按标题切成 `DocumentSection`，再按 ADR-003 的 512 token 上限拆块。本 change 不引入分词器，上限用字符预算近似。
- 同一正文再次扫描不产生第二套对象和 chunk。正文变化留下新行；删除过期行、改名与分支切换不在本 change。
- 模式升到下一版：`chunks` 增加正文列，FTS5 以 `content=` 挂在该列上，插入正文时同步可检索。`pending` 保持待索引。不调用向量库，不计算嵌入。
- CLI 增加对一个显式目录的索引命令，把扫描结果写入调用方给出的库路径。新对象附带一条规则来源的创建事件。

## Capabilities

### New Capabilities

- `text-ingest`: 目录扫描、文本与 Markdown 分块、内容哈希，以及把结果写入调用方指定库的摄取行为。

### Modified Capabilities

- `sqlite-schema`: chunk 行携带正文；FTS5 内容索引随正文写入而可检索；已有库经迁移升到含正文列的版本。
- `sqlite-infra`: 「第三方依赖最小面」从 rusqlite 与 lancedb 两系，扩大为再加 sha2 一系。

## Impact

- `mnemex-ingest`：扫描、分块、SHA-256。继续只依赖 `mnemex-core` 与 `sha2`，不依赖 `mnemex-store`（避免把 lancedb 拉进扫描测试）。
- `mnemex-store`：下一版迁移、带正文的 chunk 插入、FTS5 内容表。
- `mnemex-cli`：索引命令，并依赖 `mnemex-store` 作为组合根。
- 不改知识对象五种类型、向量补偿契约、边查询、Git 分析、四语言解析、PDF 与扫描页（ADR-012）、sidecar 与 UI。
