# ADR-004：摘要运行时与 L2 档位

## 状态

Accepted

## 背景

v3 的 L2 档位是 Qwen3-4B / 8B。v3.1 用 Qwen3.5 家族替换该档位，并把推理改成独立 sidecar。本地与云端若共用一个 OpenAI 兼容客户端，出云边界会退化成 `base_url` 字符串。

## 决策

**档位**（替换 v3 §3.3 的 Qwen3-4B / 8B 清单）：

| 档位 | 模型 | 说明 |
|---|---|---|
| 8GB 默认 | Qwen3.5-4B Q4_K_M | thinking 关闭；`n_ctx` 设远小于 262K 的硬顶 |
| 16GB+ | Qwen3.5-9B | 仅限无并发 embedding |
| 轻量 / 测速 | Qwen3.5-2B / 0.8B | 0.8B 与 2B 兼任 L1.5 主力（ADR-010） |

**任务级 token 预设**：输出 token 上限按任务给预设（摘要、抽取、决策、视觉各一档），类型上必填。缺预设则拒绝启动。不采用"必填但无默认"。

**进程级排程**：同一时刻只驻留 embedding、L1.5、L2 三者之一，切换时卸载。Phase 0 按此重测设备矩阵；不过线则 8GB 默认降到 2B，并关闭视觉。

**运行时**：

- `SummaryProvider` / `DecisionProvider` 的本地默认实现是 OpenAI 兼容客户端加本地 sidecar。进程内 FFI 后置。
- MVP 只绑定 **llama.cpp server**。受约束解码是 provider 私有协商，GBNF 留在 llama.cpp 适配器内部，不进引擎。`DecisionProvider` 的契约是 JSON schema 进、类型出、失败回退 L0。
- Ollama 只用于探测用户已有实例：支持受约束解码才登记为 `DecisionProvider`，否则只作 `SummaryProvider`。mistral.rs 留在观察名单，不绑定。
- sidecar 生命周期的最小子集（进程生命周期、端口冲突、僵尸进程）前移到 Phase 0 退出标准。

**出云门**：共享 HTTP 客户端默认只允许 loopback。非 loopback 必须同时满足：L3 总开关开启、路径不在"禁止出云"列表、UI 二次确认。

## 备选方案

- 三个后端靠换 endpoint 互通：受约束解码并不互通。
- 默认允许任意 `base_url`：出云从默认关闭变成默认打开。

## 后果

- v3 的"L1/L2 互斥加载"升级为进程级排程。旧的设备矩阵数字作废，Phase 0 重测。
- 回归集增加"thinking 误开"与"预设缺失"用例。

## 引用

- v3 §3.3（被本节档位替换的运行时分工仍有效：embedding 走 ort，摘要走 GGUF）
- v3.1 §二
- v3.1 §三
- v3.1 §十一（误配 base_url 出云）
