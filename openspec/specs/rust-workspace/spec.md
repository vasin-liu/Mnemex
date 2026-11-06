# rust-workspace Specification

## Purpose
约束 Mnemex Rust workspace 的构建健康、crate 单向依赖、MSRV/edition 统一声明与第三方依赖准入：任何 change 不得使全量构建/测试变红，`mnemex-core` 保持零内部依赖，两个 adapter 互不依赖；脚手架期零第三方依赖，此后新依赖须以独立 change 提出且默认层仅接受 Apache-2.0/MIT。

## Requirements

### Requirement: Workspace 构建健康

Rust workspace SHALL 在主流工具链（≥ MSRV 1.94）上保持全量构建与测试通过，任何 change 不得使 `cargo build --workspace` 或 `cargo test --workspace` 变红。

#### Scenario: 全量构建与测试

- **WHEN** 在仓库根目录执行 `cargo build --workspace` 并随后执行 `cargo test --workspace`
- **THEN** 两条命令均以退出码 0 结束

#### Scenario: Clippy 基线

- **WHEN** 执行 `cargo clippy --workspace`
- **THEN** 无 error 级诊断（warn 以 workspace.lints 声明的基线为准）

### Requirement: Crate 依赖方向

Workspace SHALL 维持单向依赖：`mnemex-core` 不依赖任何内部 crate；各 adapter（mcp/http）互不依赖；外层 crate（ingest/retrieval/adapter/cli/store）仅向内依赖。

#### Scenario: core 零内部依赖

- **WHEN** 执行 `cargo tree -i mnemex-core` 检查反向引用
- **THEN** 除被依赖外，`mnemex-core` 自身依赖清单不包含任何 `mnemex-*` 内部 crate

#### Scenario: adapter 互不依赖

- **WHEN** 分别查看 `mnemex-adapter-mcp` 与 `mnemex-adapter-http` 的依赖清单
- **THEN** 两者互不出现，仅依赖 core 侧共享接口（对应 v3.1 §5.3"两协议只共享 Engine 接口"）

### Requirement: MSRV 与版本声明

Workspace SHALL 统一声明 `rust-version = "1.94"` 与 `edition = "2024"`，取值与 ADR-004/LanceDB MSRV 记录一致。

#### Scenario: 声明一致性

- **WHEN** 读取根 `Cargo.toml` 的 workspace 级配置
- **THEN** `rust-version` 为 1.94 且各成员 crate 继承该值，不出现成员级覆盖

### Requirement: 依赖准入红线

脚手架阶段 Workspace SHALL 不引入任何第三方依赖；此后任何第三方依赖引入 SHALL 满足许可证红线（默认层 Apache-2.0/MIT only，对应 ADR-003）且以独立 change 提出。

#### Scenario: 零第三方依赖验收

- **WHEN** 在本 change 验收时执行 `cargo tree --workspace`
- **THEN** 除 `mnemex-*` 成员外依赖树为空（构建所需隐式 proc 宏除外）

#### Scenario: 引入须走 change

- **WHEN** 后续任何 change 需要新增第三方依赖
- **THEN** 该 change 的 proposal 明示依赖名、版本、许可证，且许可证满足红线
