# ADR-011：Agent 工件读写分离

## 状态

Accepted

## 背景

AGENTS.md、CLAUDE.md、SKILL.md 已是仓库内的跨工具指令文件。读这些文件和把技能写回去是两件事。合成一个 Adapter 会让采集变成对仓库的修改。

## 决策

1. **`AgentArtifactSource`（Phase 1，只读）**：默认只读仓库内的 AGENTS.md、CLAUDE.md、SKILL.md，按普通 Markdown 解析。不解析 auto-memory 的厂商目录格式。auto-memory 导入是 Phase 2 的 opt-in，目录布局另以 ADR 钉住，不进 Phase 1 关键路径。
2. **`SkillExporter`（Phase 4，显式授权）**：模式库导出为 SKILL.md / AGENTS.md 片段。默认关闭。默认写入用户本地技能目录，不写入 git 仓库。写入仓库须单次显式确认。不自动 commit。
3. **承诺计数为 3**：MCP（dual-era）、HTTP、Agent 工件。CLI 维持"Phase 2 末可选交付"，不计入承诺数。
4. **Context Bundle compaction 验收（Phase 2）**两条：
   - 引用以知识对象 id + `content_hash` + commit 为主，路径为辅；
   - compaction 之后只剩 id，仍能再次 `get_context_bundle`；截断后重新注入不改变稳定 id。
   不绑死某一客户端 `/compact` 的具体行为。

本 change 不为导出器建 crate。读路径将来落在 `mnemex-ingest`，写路径落在独立导出 crate。

## 备选方案

- 一个 Adapter 同时读写：设置上无法把"读入"和"写出"分成两个开关。
- 以路径作为 compaction 主键：compaction 之后路径可能指向新内容，绕开 freshness。

## 后果

- 写出默认关闭，且默认不进用户 git 仓库。
- 格式解析留在采集层，不进核心引擎。

## 引用

- v3.1 §五
- v3 §六（CLI 延后，不计入承诺）
