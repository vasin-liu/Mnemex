# Spec Delta

## MODIFIED Requirements

### Requirement: 第三方依赖最小面

准入 sha2 之后，workspace 直接第三方依赖 SHALL 仅为 `rusqlite`、`lancedb` 与 `sha2` 三系及其传递依赖。测试与实现 SHALL NOT 借机引入 tempfile、thiserror 或其他未准入的库。此后再新增任何依赖仍须以独立 change 提出（`rust-workspace` 依赖准入红线）。

#### Scenario: 依赖树检查

- **WHEN** 执行 `cargo tree --workspace` 并过滤出 `mnemex-*` 之外的条目
- **THEN** 仅出现 rusqlite 一系、lancedb 一系与 sha2 一系
