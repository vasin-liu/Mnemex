# Design

## Context

见 proposal.md 的 Why。现状约束：`chunks` 没有正文列；FTS5 只在初始化时做临时表冒烟；`insert_chunk` 只写对齐键；`mnemex-ingest` 三个模块是空注释；CLI 只有 `--help` / `--version`，且不依赖 `mnemex-store`。`heading_path` 已定为 `/` 级联（store schema D8）。生产路径继续走 `Result`，不新增错误库。

## Goals / Non-Goals

**Goals:**

- 扫描与分块留在 `mnemex-ingest`，落库留在 `mnemex-store`，CLI 做组合。
- 模式从 v1 升到 v2：正文列 + 外部内容 FTS5，旧库打开时自动迁移。
- 未改动的目录再索引一次，对象数与 chunk 数不变。
- 新 chunk 保持 `pending = 1`，索引命令不打开向量库。

**Non-Goals:**

- 见 proposal 的范围。此处再钉死三条实现边界：不解析 Setext 标题；不读 `.gitignore`；正文变更后的旧 chunk 留在库里，不在本 change 删除。

## Decisions

### D1 `sha2` 只进 `mnemex-ingest`

直接依赖 `sha2 = 0.11.1`，默认 feature。`digest` 与 `cfg-if` 算 sha2 一系。哈希结果用原始 32 字节，不引入 hex 编码库。备选：把 `sha2` 放进 `mnemex-store`——扫描测试会因此编进 rusqlite，否决。备选：自写 SHA-256——没有收益，否决。

### D2 摄取不依赖 store

`mnemex-ingest` 返回纯数据：相对路径、章节标题、`heading_path`、chunk 正文、内容哈希、对象 id 与 chunk id。`mnemex-cli` 打开 `SqliteStore` 并写入。CLI 新增对 `mnemex-store` 的依赖。备选：让 ingest 依赖 store——会把 lancedb 拉进每次扫描测试，否决。

### D3 标识从路径和正文派生

内容哈希是该 chunk 正文 UTF-8 字节的 SHA-256。对象 id 取 SHA-256(相对路径 ‖ `/` ‖ 标题路径) 的前 16 字节。chunk id 取 SHA-256(对象 id ‖ 序号 ‖ 内容哈希) 的前 16 字节。相对路径的分隔符统一写成 `/`，避免 Windows 反斜杠改变 id。对象上的 `content_hash` 是整节正文（拆块之前）的 SHA-256；`commit` 留空。`created_at` 用文件修改时间的 Unix 毫秒。已存在的对象 id 不再插入，也不再追加创建事件；已存在的 chunk id 不再插入。备选：随机 16 字节——同一次内容无法幂等，否决。

### D4 分块规则

扩展名比较忽略大小写。拒绝名单按路径段精确匹配 `.git`、`target`、`node_modules`。根目录不存在或不是目录时，扫描失败。单个文件打不开时，扫描失败。能打开但不是合法 UTF-8 时，跳过该文件。

ATX 标题是行首 1 到 6 个 `#` 加空格。围栏代码块（成对的 ` ``` `）内部的 `#` 不算标题。无标题文件的章节标题用文件名（不含扩展名），标题路径就是该标题。空文件不产生对象，也不产生 chunk。

超长正文按 Unicode 标量值取窗，窗口上限 512。窗口内有换行时，在最后一个换行之后切开，换行留在前一块；窗口内没有换行则在第 512 个标量值处切开。拼接必须等于原正文，不插入分隔符。备选：引入分词器按 token 切——那是新的依赖 change，否决。

### D5 v2 迁移与外部内容 FTS

`MIGRATIONS` 追加一条：`chunks` 增加 `body TEXT NOT NULL DEFAULT ''`；创建 `chunks_fts`，`USING fts5(body, content='chunks', content_rowid='rowid', tokenize='trigram')`；插入、删除、更新三支触发器把 `body` 同步进 FTS；最后 `INSERT INTO chunks_fts(chunks_fts) VALUES('rebuild')`。整段在现有迁移事务里执行，`user_version` 变为 2。

`insert_chunk` 增加正文参数。现有只为对齐键服务的调用点传入空串。新增按 chunk id 读正文，以及按 FTS 查询返回 chunk id。trigram 查询至少 3 个字符；规格场景与测试用不少于 3 个字符的词。备选：`unicode61`——中文没有空格，整段会变成难以按词命中的长 token，否决。备选：正文另表、FTS 再拷一份——两份正文会漂移，否决。

### D6 索引命令

`mnemex index --dir <目录> --db <库文件>`。两个参数都缺一不可；缺参数时退出失败，且不创建库文件。参数解析仍用 std。命令只调用 `SqliteStore` 的打开与写入，不调用 `LanceVectorStore::open`。新对象的创建事件来源为 `EventSource::Rule`。生命周期用默认值：`freshness = Fresh`，`confidence` 与 `model_confidence` 皆无。

### D7 错误类型

`mnemex-ingest` 用手写错误枚举表达「根目录不可用」「文件不可读」。不使用 `unwrap` / `expect`，不引入 thiserror。

## Risks / Trade-offs

- [512 个标量值比英文的 512 token 更短] → 仍落在 ADR-003 的上限之内。真正的 token 计数留到嵌入 change。
- [trigram 不命中 1–2 字的查询] → 本 change 的检索场景使用不少于 3 个字符的词。
- [正文改了以后旧 chunk 还在，待索引位也还在] → 删除与对账是后续 change。在此之前向量重建会看见这些旧行，调用方仍须自备向量。
- [固定拒绝名单盖不住仓库自己的忽略规则] → 完整 gitignore 不在本 change。
- [围栏检测只认成对的三反引号] → 缩进代码块里的 `#` 仍可能被当成标题。文本摄取先覆盖 ATX 与围栏这一种常见写法。

## Migration Plan

没有已发布的用户库。开发库在下次打开时从 v1 升到 v2，旧 chunk 的正文变为空串。回滚是 revert 本 change；已升到 v2 的库文件删掉重建即可，程序会拒绝比自己新的 `user_version`。

## Open Questions

（无。标识派生、分块上限和分词器都已经写进上面的决定，不再留到实现时改规格。）
