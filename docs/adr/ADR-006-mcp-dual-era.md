# ADR-006：MCP dual-era 与 scope

## 状态

Accepted

## 背景

2026-07-28 规范的核心是无状态，但 2026-09/10 的主流客户端仍发送 2025-11-25 的 `initialize`。legacy 客户端对 modern-only 服务端没有前向回退。规范兼容矩阵写明这种组合会失败。

## 决策

- **主路径是 2026-07-28 无状态**：`server/discover`、`_meta` 版本协商、无会话。同时保留 `initialize` 以兼容旧客户端。
- legacy 路径的拆除日期与规范的 12 个月弃用窗口、目标客户端兼容矩阵对齐。**不在 Phase 2 拆除**。
- 所有响应携带 `resultType`。需要客户端补充信息时走 MRTR（Multi Round-Trip Requests）。不再依赖已弃用通知（如 `roots/list_changed`）。
- 引擎内部只保留一套无状态调用语义。两个版本的差异封在 Adapter 内。
- **Client-supplied scope**：工作区路径，加可选分支或提交（`scope_id`），是普通工具参数，每次调用由客户端显式传入。服务端不为此铸造 handle。
- **Server-minted handle** 只用于客户端构造不出的服务器状态：索引快照、分页游标、Phase 3 订阅、Tasks 任务句柄。铸造、过期与作废规则留在 Adapter，不进 Memory Engine。
- "scope_id 直接映射为 server-minted handle"作废。
- Tasks 已是官方扩展（`io.modelcontextprotocol/tasks`）。`subscriptions/listen` 承载的是工具与资源列表变更，不是语义推送。
- Phase 3 主路径是本地 UI 角标加拉取式 tool。3a 为承诺交付的拉取，3b 为可选的规范推送。MCP Tasks 用于可取消的长任务。
- HTTP API 与 MCP Adapter 分离，只共享 Engine 接口。不借协议头把两边合并。

## 备选方案

- 只实现 2026-07-28：现网旧客户端无法连接。
- 把 scope 做成服务端会话：无状态查询多一轮往返，会话状态换了个名字。

## 后果

- `mnemex-adapter-mcp` 与 `mnemex-adapter-http` 互不依赖。
- 规范 URL、章节号与目标客户端兼容矩阵在本 Adapter 的实现 change 里补权威引用。本 ADR 固定原则，不抄会过时的条文细节。

## 引用

- v3 §2.1
- v3 §5.1
- v3 §5.2
- v3 §5.3
