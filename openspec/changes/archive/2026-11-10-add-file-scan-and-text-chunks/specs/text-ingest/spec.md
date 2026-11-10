# Spec Delta

## Purpose

把调用方指定目录里的文本和 Markdown 收成文档章节与 chunk 正文，写入指定的库，并保持这些 chunk 待索引。

## ADDED Requirements

### Requirement: 文本文件扫描

摄取 SHALL 递归扫描调用方给出的目录，只收录扩展名为 `.md`、`.markdown`、`.txt` 且能按 UTF-8 解码的文件。路径中含 `.git`、`target` 或 `node_modules` 目录段的条目 SHALL 跳过。其余文件 SHALL 跳过，且不使本次扫描失败。

#### Scenario: 收录 Markdown 与纯文本

- **WHEN** 目录中同时有 `.md`、`.txt` 与 `.rs` 文件
- **THEN** 前两者进入摄取结果，`.rs` 不进入

#### Scenario: 跳过拒绝名单

- **WHEN** 文件位于 `.git`、`target` 或 `node_modules` 目录之下
- **THEN** 这些文件不进入摄取结果

#### Scenario: 非 UTF-8 不中断扫描

- **WHEN** 同一目录中有一份非 UTF-8 文件和一份 UTF-8 的 `.txt`
- **THEN** 扫描成功，只收录那份 `.txt`

### Requirement: 章节与长度分块

Markdown SHALL 按 ATX 标题切成文档章节，记录该节标题，并用 `/` 连接从文档根到该节的标题路径。无标题的文本 SHALL 成为一个章节。章节正文 SHALL 再拆成若干 chunk：每个 chunk 的 Unicode 标量值数量不得超过 512，按顺序拼接等于原正文。空文件 SHALL NOT 产生 chunk。

#### Scenario: 标题切成章节

- **WHEN** 一份 Markdown 含两个一级标题，且第二个标题下有一个二级标题
- **THEN** 产生对应的文档章节，二级节的标题路径为两级标题以 `/` 连接

#### Scenario: 超长正文再拆

- **WHEN** 某一节正文的 Unicode 标量值数量超过 512
- **THEN** 该节对应多个 chunk，每个不超过 512 个标量值，按顺序拼接后与原正文一致

#### Scenario: 空文件无 chunk

- **WHEN** 扫描一份空的 `.txt`
- **THEN** 不产生该文件的 chunk

### Requirement: 内容哈希与重复扫描

每个 chunk SHALL 带 32 字节的 SHA-256 内容哈希。对内容未改动的同一目录再次摄取 SHALL NOT 增加知识对象或 chunk 的数量。

#### Scenario: 相同内容再次扫描

- **WHEN** 对同一目录成功摄取后，文件内容不变，再次摄取
- **THEN** 知识对象数量与 chunk 数量都不变

#### Scenario: 不同正文哈希不同

- **WHEN** 两个 chunk 的正文不同
- **THEN** 它们的内容哈希不同，且长度都是 32 字节

### Requirement: 落库并保持待索引

摄取 SHALL 把新的文档章节及其 chunk 正文写入调用方指定的库。每个新对象 SHALL 附带一条来源为规则的创建事件。新 chunk SHALL 处于待索引。本次摄取 SHALL NOT 写入向量索引。

#### Scenario: 正文落库且待索引

- **WHEN** 对含一份短 Markdown 的目录执行摄取
- **THEN** 库中能按该正文中的词经全文检索命中对应 chunk，且该 chunk 为待索引

#### Scenario: 创建事件来自规则

- **WHEN** 摄取产生一个新的文档章节
- **THEN** 该对象存在一条创建事件，来源为规则

#### Scenario: 不写入向量索引

- **WHEN** 摄取完成
- **THEN** 这些 chunk 仍为待索引，且不存在已写入的向量对齐键

### Requirement: 显式目录与库路径

索引入口 SHALL 要求调用方同时给出待扫描目录与库路径。任一缺失时 SHALL 失败，且不创建库文件。

#### Scenario: 缺少目录

- **WHEN** 索引命令未提供待扫描目录
- **THEN** 命令失败，且不创建数据库文件
