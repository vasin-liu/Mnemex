# ADR-001：向量库首选、回退与四项退出标准

## 状态

Accepted

## 背景

sqlite-vec 自 2026-05-18 起无提交，稳定版停留在 0.1.9，README 自声明 pre-v1。LanceDB 活跃（Rust crate，Apache-2.0，MSRV 1.94）。上游活跃度只能说明 sqlite-vec 不宜作为默认首选，不能代替本产品在桌面场景（双库一致性、Windows 文件锁、打包体积、增量更新）上的验证。

## 决策

1. **LanceDB 定为 Phase 0 实现首选，sqlite-vec 保留为回退候选**，不设为永久弃用。
2. Phase 0 做一次单候选 Spike（约一周，中等规模仓库）。四项退出标准：
   - 文件增、删、改名、分支切换时，SQLite（FTS/edge）与 LanceDB 向量的一致性成立，补偿逻辑可行；
   - Windows 上 Tauri 打包、Defender 与文件锁场景可正常运行；
   - 10 万级 chunk 增量重建的停顿可接受；
   - 备份与恢复是用户可复制的单目录操作。
   任一项不过，切回 sqlite-vec 回退实现。Spike 结论写入后续记录。
3. 存储边界是 `VectorStore` trait。LanceDB 是默认实现，不是调用边界。
4. BM25 关键词检索只来自 SQLite FTS5。LanceDB 只做向量。RRF 在引擎层做，避免两套 BM25。

双库如何对齐见 ADR-002。

## 备选方案

- 把 sqlite-vec 直接定为永久弃用：活跃度不能证明 LanceDB 在本机桌面场景可用。
- 不做 Spike、按上游活跃度冻结 LanceDB：桌面文件锁与增量更新未验证。

## 后果

- 向量库实现可替换。Spike 不过不阻塞 Phase 1，只是换回退实现。
- 风险表同时记下 LanceDB 的版本 churn（API 抖动与 MSRV 门槛）和 sqlite-vec 的停滞。

## 引用

- v3 §3.1
- v3 §2.2（图不进向量库；SQLite edge 维持）
