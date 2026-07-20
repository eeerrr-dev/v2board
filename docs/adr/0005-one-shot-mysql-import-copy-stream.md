# 0005. 一次性 MySQL 导入:只读快照 + COPY 流式,失败即弃重来

- 状态:已采纳(回填)
- 日期:2026-07(回填;`v2board-lifecycle` 已实现并被门禁覆盖)

## 背景(Context)

native 新版**尚未发布**:没有已安装的 native 数据库,没有升级契约要背。唯一
的历史包袱是旧站的 Oracle MySQL 8 业务数据。约束:

- 可以接受一次停机窗口 —— 这是自营小站,不是不能停写的多租户平台;
- 旧数据库绝不能被改动(它是最后的退路);
- 单操作者没有能力运维 CDC 管道或双写一致性;
- 旧机与新 PostgreSQL 在同一机房私网,带宽不是瓶颈。

## 决策(Decision)

只有一条路径 `mysql-import.v1`,执行器是一次性 CLI `v2board-lifecycle`
(`validate` / `inspect` / `execute`):

- 停写旧站 → 完整 dump 仅作受保护备份(**不是 converter 输入**)→ 在旧机用
  专用 `SELECT`-only 账号建立 server-enforced `REPEATABLE READ`、`READ ONLY`
  一致性快照;额外 grant/role/`GRANT OPTION` 拒绝,14 张 imported 表必须 InnoDB。
- 每张源表一条主键有序 streaming `SELECT`,逐字段显式校验转换后,每张目标表
  **恰好一条 PostgreSQL `COPY FROM STDIN`** 经同机房私网认证 TLS 流入全新空
  cluster;内存上限 = 当前行 + 有界发送缓冲 + ≤4096 项 Stripe payment-id 索引;
  不落中间 COPY/CSV 文件。
- 全部 COPY 后统一建唯一约束/索引/外键、reset sequence、`ANALYZE`,再按主键序
  对每张保留表做**且只做一次**整表 canonical scan,与转换时累计的 expectation
  对比。
- **失败即弃重来**:删掉不完整的新 PostgreSQL/ClickHouse/Redis target 和输出
  目录,修正问题,对全新空 target 再跑一次。没有 resume、checkpoint、rollback、
  recovery 或 cleanup/restart 状态机 —— 旧库从未被动过,"回滚"无从谈起。

**明确拒绝的替代方案:**

- **CDC / 双写 / 兼容窗口 / 渐进切流** —— 为一次性的、可停机的迁移引入常驻
  一致性机器,复杂度与风险完全不成比例。
- **批量 `INSERT`(如固定 1000 行 batch)或可选传输模式** —— COPY 是唯一传输
  路径;第二条路径意味着两套校验语义。
- **断点续传 / 恢复状态机** —— 可恢复中间状态本身就是一类要测试的持久化产品;
  重跑一次空导入比证明"部分导入 + 续传 = 完整导入"便宜得多。
- **把 dump 灌给 PostgreSQL / staging MySQL** —— MySQL SQL 永不发给 PostgreSQL,
  转换只走 typed row。

## 后果(Consequences)

- 得到:导入器零持久状态、幂等性论证极简("空 target + 完整跑一次");旧库
  全程只读,任何失败的最坏结果都是浪费一次停机窗口;三个 SHA-256(dump/
  源 schema/converted snapshot)各证其事,不互相冒充。
- 代价:切换需要**真实停机**,窗口长度受数据量约束且不可分段;导入失败必须
  从头重跑(接受)。
- 代价:固定丢弃是真损失 —— 旧 Redis 全部状态、Stripe 配置与未完成 Stripe
  订单、旧节点/路由/凭据、明细日志表全部不进新版,且**不可按次配置**;全员
  重新登录,节点手工重建。
- 代价:该路径只服务"首发前从 MySQL 来"这一种场景;未来 native 升级契约要
  在真实发布后另行设计,不得提前掺进来。

## 证据

- [`../mysql-import-invariants.md`](../mysql-import-invariants.md) — 不可变契约
  (流程、14 表映射、Stripe 规则、丢弃边界)。
- [`../mysql-import.md`](../mysql-import.md) — 操作指南;
  [`../examples/mysql-import.v1.example.json`](../examples/mysql-import.v1.example.json)
  — 唯一清单示例。
- [`../../backend/rust/crates/lifecycle/src/`](../../backend/rust/crates/lifecycle/src/)
  与
  [`../../backend/rust/crates/provision/src/mysql_import_converter.rs`](../../backend/rust/crates/provision/src/mysql_import_converter.rs)
  — CLI 与固定转换器实现。
- [`../postgresql-clickhouse-invariants.md`](../postgresql-clickhouse-invariants.md)
  §7 — 初始数据库与导入的持久化侧约束。
- [`../../AGENTS.md`](../../AGENTS.md) — Pre-Release MySQL Import Direction。
