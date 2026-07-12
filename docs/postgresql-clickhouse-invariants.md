# PostgreSQL + ClickHouse 持久化不变量

状态：**F0 架构已冻结；native runtime 与分析投影已实现，lifecycle converter/apply 仍 fail closed**

本文件取代“新版运行时以 MySQL 8.4 为目标”的设计。旧版来源仍是
`references/wyx2685-v2board` 所代表的 MySQL/MariaDB 系统；新版运行时不再把
MySQL 或 MariaDB 作为可选后端。

在 PostgreSQL 移植、ClickHouse 投影、迁移转换器和对应门禁全部通过之前，
lifecycle `apply` 必须继续不存在，现有检查必须继续 fail closed。

## 1. 固定产品选择

- 新版唯一权威事务库是 PostgreSQL 18，运行时接受当前 18.x 安全补丁版，不自动跨
  PostgreSQL major 升级。
- 新版分析库是 ClickHouse 26.3 LTS，运行时接受仍获安全更新的 26.3.x 补丁版，不跟随
  monthly innovation release 自动升级。
- Redis 只保存 session、限流、lease、锁和短期缓存，不保存不可恢复账本。
- 不引入 TimescaleDB、Kafka、Redpanda、RabbitMQ、Elasticsearch/OpenSearch 或第二套
  事务数据库。只有经过容量门禁证明 SQL outbox 无法满足要求后，消息流才可作为独立
  架构变更提出。
- 旧版 source adapter 可读取经明确识别和测试的 MySQL/Percona 或 MariaDB；source
  vendor 不改变 PostgreSQL target。

初始本地镜像固定 PostgreSQL 18.4 和 ClickHouse 26.3 LTS 的最新安全补丁。镜像还必须
固定内容 digest；补丁升级通过正常依赖更新和回归门禁完成，不能使用浮动 `latest`。

## 2. 数据所有权

| 数据 | 唯一权威位置 | ClickHouse 是否允许保存 |
|---|---|---|
| 用户、订阅、套餐、额度、`u/d`、`traffic_epoch` | PostgreSQL | 只允许不可反写的维度快照 |
| 订单、支付、余额、佣金、优惠券、礼品卡、reconciliation | PostgreSQL | 只允许脱敏分析投影 |
| 待处理流量报告、payload hash、幂等状态、worker claim | PostgreSQL | 否 |
| auth token、session epoch、节点凭据、配置 | PostgreSQL | 否 |
| 已结算的不可变流量事件 | PostgreSQL outbox 先提交 | 是，作为可重建事实表 |
| 小时/日/用户/节点/套餐/费率聚合 | PostgreSQL 保留产品所需近期权威摘要；ClickHouse 保存分析投影 | 是 |
| migration、installation、upgrade、operation ledger | PostgreSQL | 否 |

任何 ClickHouse 数据都必须能够从仍受控的 PostgreSQL outbox/归档重新投影。删除整个
ClickHouse database 不得改变订单、支付、用户额度、订阅资格或幂等结论。

## 3. 事务和发布边界

一次流量报告只有在同一 PostgreSQL 事务完成以下动作后才算 `applied`：

1. 锁定并验证 report、user、node、group 和 `traffic_epoch`；
2. 仅一次更新用户权威额度和近期权威统计；
3. 完成 report 幂等状态；
4. 为每个实际应用的用户增量插入 typed analytics outbox row；
5. 提交事务。

API 和结算 worker 禁止同步双写 ClickHouse。PostgreSQL 提交失败时不得出现 ClickHouse
事件；ClickHouse 不可用时 PostgreSQL 事务仍可提交，未发布 outbox 必须保留并重试。

流量分析固定为两个不可变 event major，不能把二者折叠成一个含义含混的 `traffic`：

- `traffic.reported.v1`：API 已验证并耐久接受的原始报告事实；
- `traffic.accounted.v1`：worker 已锁定 epoch 并完成额度结算的结果，逐 item 标记
  `applied|stale_epoch|missing_user`。

每条 item event 固定包含：

- `event_id`：小写 SHA-256 hex，由 event name、canonical installation UUID、`report_key` 和 canonical
  decimal `user_id` 按具名 v2 domain separator 确定性生成；
- `event_name`、`schema_major`、installation identity；
- `report_key`、`payload_hash`、`identity_kind=explicit|implicit`；
- `user_id`、`traffic_epoch`、`server_id`、`server_type`；
- rate 的原始十进制文本和用于旧统计语义的 `DECIMAL(10,2)`；
- 非负 raw `u/d` 以及由 PostgreSQL/Rust 按现行 MidpointAwayFromZero 规则算好的
  charged `u/d`；ClickHouse 禁止用 Float 重新计费；
- `accepted_at`、固定的 `accounting_date`、`Asia/Shanghai` 业务时区；accounted event
  另含 `accounted_at`、outcome 以及需要时的权威 `u_after/d_after`。

report identity 保持现有 `SHA256(node_type\0node_id\0client_token)` 规则。无客户端幂等键的
旧节点继续使用显式标注的 implicit identity；不能按“payload 看起来一样”擅自去重。所有
超过 JSON/JavaScript 精确整数范围的 ID、epoch、byte counter 和 decimal 在 canonical event
payload 中使用十进制字符串，并对 canonical bytes 计算 `payload_sha256`。

outbox relay 必须：

- 通过 `FOR UPDATE SKIP LOCKED` 有界 claim；
- 在第一次 claim 时持久化 immutable `delivery_batch_id`、目标 table generation、单一月份
  partition、row count、content hash、insert settings hash 和稳定行顺序；
- 使用相同 batch id、相同内容和相同顺序重试不确定的 ClickHouse acknowledgement；
- 仅在 ClickHouse 确认落盘后标记 `published_at`；
- 支持 lease 丢失、进程崩溃、重复发送、乱序、ClickHouse 恢复和全量 replay；
- 不因 ClickHouse 故障阻塞订单、认证或支付线程。

在长期 archive、generation replay 与 restore drill 完成前，PostgreSQL 的 terminal published row
不得由 runtime 自动清理；pending 与 quarantined 更不得删除。显式 prune API 只供隔离测试，不能进入
生产调度。未来启用任何 replay window 都属于破坏性变更，必须先证明第二份恢复来源、容量预算和恢复
时间，并展示将失去的永久 event-id 冲突证据。

不得把 ClickHouse `ReplacingMergeTree` 后台 merge 当作计费幂等。生产 raw 表使用普通
append-only MergeTree/ReplicatedMergeTree；relay 对不确定 acknowledgement 必须按 batch
manifest 查询并核对全部 event id/content hash。部分写入或冲突属于完整性事故，需要隔离并
重建目标月 generation，不能补几行后假装成功。产品权威数字仍从 PostgreSQL 读取。

## 4. PostgreSQL schema 方向

- 为 PostgreSQL 建立全新的 SQLx migration lineage。旧 43 个 MySQL migration 的版本和
  checksum 不得伪装成已经在 PostgreSQL 执行。
- 新装从 PostgreSQL final-state baseline 开始；旧 MySQL/MariaDB 到 PostgreSQL 由独立
  converter 完成。
- 所有高增长 identity、用户和关联外键采用 `BIGINT`；金额继续使用整数 cents，并有
  非负/范围 check。
- 状态枚举使用 `SMALLINT` + check，真正二值字段使用 `BOOLEAN`。
- 可变配置使用 `JSONB`；可查询或参与约束的关键字段保持普通列。
- 未完成订单、open ticket 等条件唯一性使用 PostgreSQL partial unique index，不复制
  MySQL generated-column workaround。
- token、trade number、callback identity、event id 等机器标识采用确定的 byte/case
  语义；email 在写入边界规范化并以规范值唯一。
- 从旧 `utf8mb4_unicode_ci` 迁移前必须检查大小写、重音、全半角、尾空格和 Unicode
  normalization 碰撞。不能用 `citext` 或 `lower()` 假装与旧 collation 完全等价。
- runtime 默认 `READ COMMITTED`。依赖唯一约束解决“空范围”并发；需要更强隔离的事务
  必须对 SQLSTATE `40001`/`40P01` 做整个事务的有界 jitter retry。
- PostgreSQL DDL 尽量事务化；`CREATE INDEX CONCURRENTLY` 等不能放入普通事务的操作必须
  是显式 non-transactional migration，具有失败恢复和重复执行证明。

## 5. ClickHouse schema 方向

- reported/accounted raw 使用两张月分区的 MergeTree/ReplicatedMergeTree 表；`ORDER BY`
  首先服务用户时间范围查询，并保留稳定 `event_id`、ingest batch 和 batch row number。
- 小时/日、节点、套餐、费率和全局排行使用独立聚合表，reported 与真正 applied charged
  指标不得混算。增量 materialized view 只能建立
  在已证明重复投递不会重复计数的输入契约上。
- 原始事实 TTL 和聚合 TTL 分开配置。降低 retention 属于破坏性配置更新，必须先展示将
  删除的分区范围并二次确认。
- ClickHouse migration 有独立 schema ledger；不能借用 PostgreSQL `_sqlx_migrations`。
- schema 演进采用 additive column / dual projection / backfill / verified swap / delayed drop，
  不能原地依赖大规模 mutation。
- ClickHouse 专用 analytics reader、outbox writer 和 schema migrator 使用不同的最小权限 principal；
  relay writer 仅有 raw table INSERT 与核对自身 immutable batch 所需的受限 SELECT，不拥有 DDL。
  schema principal 只具有目标表 DDL、schema 校验所需 system metadata SELECT，以及 migration ledger
  的 SELECT/INSERT；不能误写成不可能运行 migrator 的“纯 DDL-only”。
  当前 API 不查询 ClickHouse，因此 reader 不物化到 API/worker runtime；bootstrap/schema 也只供
  一次性 lifecycle/schema job。
- PostgreSQL outbox 当前保留完整 replay history；只有经过恢复演练的 ClickHouse replicated backup
  或不可变对象归档成为第二恢复来源后，才允许提出有界 replay window。若 raw event 被定义为长期
  审计证据，还必须写不可变对象归档。不能在没有第二恢复来源时清理权威重放记录。

## 6. 连接和权限

目标 PostgreSQL 至少区分：

- bootstrap principal：只用于创建 database/roles，完成后撤离；
- migration principal：拥有 schema DDL 和 migration ledger；
- API principal：只拥有 API 所需 DML；
- worker principal：拥有 worker、queue 和 outbox 所需 DML。

PostgreSQL role 没有 MySQL `'user'@'host'` 语义。客户端来源限制由 `pg_hba.conf`、托管网络
ACL、安全组或私网控制；provision spec 必须记录该外部控制的声明和证据，不能假称 SQL
`CREATE ROLE` 已限制来源。

目标 ClickHouse 至少区分 bootstrap、schema migrator、outbox writer 和预留给专用分析消费者的
analytics reader；当前常驻进程只有 worker 得到 writer。
所有生产 PostgreSQL、ClickHouse 和 Redis 连接都必须使用经过主机身份验证的 TLS 或在
受控私网加密隧道内；开发环境例外只能存在于 Docker 内部网络。

## 7. 新装、旧版迁移和升级

### 全新安装

1. 在线只读检查 PostgreSQL 18、ClickHouse 26.3 LTS、Redis 和外部网络控制；
2. 展示将创建的 database、roles、schema、ClickHouse database/users 和 retention；
3. 二次确认后 bootstrap；
4. 运行 PostgreSQL 与 ClickHouse 各自 migration；
5. 验证最小权限、schema checksum、outbox round trip、备份/PITR 和 ClickHouse replay；
6. 从单一 manifest 生成 API/worker 两个最小 file-only runtime config，分别写入对应 Unix 用户的
   `0700` 独立目录和 `0600` 文件，再原子启用。

### 旧版迁移

1. source MySQL/MariaDB 和旧 Redis 只读在线预检；
2. 明确进入唯一维护窗口后同时停止旧 API、worker、scheduler、全部 node reporter 和临时链接签发，
   排空 pending traffic/queue；
3. 对 source 做一致快照，在所有旧/新服务均保持停机时一次性 bulk 转换到全新 PostgreSQL target；
4. 为 sequence、行数、关系、自然键、金额、token、collation 和业务不变量做双向校验；
5. 旧 `v2_stat_user` 只能迁移为已知粒度的历史聚合，不能伪造成 raw traffic event；
6. ClickHouse 从已标注 `legacy_aggregate` 的投影或新系统 applied events 建立，不能反向决定
   PostgreSQL 迁移成功；
7. 离线批量写入全部节点 scoped credential/idempotency 配置；所有节点验证完成前不恢复任何 reporter；
8. 最终一次性提交 installation、统一启动新 API/worker/reporter 并全量登出；随即撤销 source 凭据、
   隔离网络并永久停服 MySQL/MariaDB 与旧 Redis。只保留带 SHA-256 的加密冷归档，禁止运行时回退。

copy journal 和 chunk checkpoint 只允许恢复这一次维护操作的进度，不能形成 CDC、双写、shadow read、
长期 bridge 或允许旧/新系统同时服务。最终提交之后只能恢复 PostgreSQL/ClickHouse 或向前修复，不能
重新启动 MySQL。

### PostgreSQL / ClickHouse 升级

- PostgreSQL minor、major 和 ClickHouse LTS patch/next-LTS 是三类独立计划，不允许同一
  变更窗口同时跨两个数据库 major/LTS。
- PostgreSQL major 使用 `pg_upgrade` 或经过验证的逻辑/复制切换路径；必须验证扩展、
  collation version、statistics、sequence、PITR 和 rollback window。
- ClickHouse 升级先验证 outbox pause/replay、replica/merge backlog、schema compatibility
  和旧 reader；失败时停止投影，不回滚 PostgreSQL 事务。
- 删除列、缩短 TTL、改变 event identity、排序/分区键或聚合语义均属于破坏性升级。

## 8. 故障和健康语义

- PostgreSQL 不可用：核心 API/worker 不 ready，禁止以 Redis 或 ClickHouse 代替权威读写。
- Redis 不可用：依赖 session/lease 的功能按既有 fail-closed 契约降级。
- ClickHouse 短期不可用：订单、认证、支付和流量结算继续；analytics 标记 stale/unavailable，
  outbox 积压并告警，禁止回退为对 PostgreSQL 的无界分析扫描。长期故障必须在预先冻结的 PG
  rows/bytes/磁盘水位触发安全背压；容量策略未实现前不能宣称任意时长的 CH 故障不影响核心。
- 本地容器 stdout/stderr 与数据库内部日志必须同时有界轮转；Compose 固定为 `local` driver、
  `10m × 3`。裸机生产使用有界 journald 和各数据库原生日志轮转，日志耗尽共享磁盘按
  PostgreSQL/ClickHouse 不可用告警。
- analytics 响应必须携带 freshness/lag；不得把陈旧投影伪装成实时权威数据。
- outbox oldest age、pending rows/bytes、publish rate、retry count、ClickHouse parts/merge、
  PostgreSQL WAL/vacuum/lock、replica lag 和 restore time 都是发布门禁指标。

## 9. 实现状态与剩余门禁

已完成并由代码门禁覆盖：

- native Rust runtime、SQLx concrete types、SQL 和 final-state baseline 全量切换 PostgreSQL；
- PostgreSQL fresh migration、374 条 live SQL prepare、并发业务 invariant 与 worker reconciliation；
- typed analytics outbox、immutable batch、ClickHouse 26.3 version/schema gate、lost-ACK retry、完整核对、
  quarantine、默认不清理的 replay history 和真实 PostgreSQL→ClickHouse 往返；
- Docker 本地 PostgreSQL/ClickHouse/Redis、固定 image digest 和隔离 integration target；
- strict manifest/report schema v3，以及 fresh/legacy/native-upgrade 三条两阶段只读流程；
- 现行文档和 CI 中的 target-MySQL/v2 指令退役。

以下能力尚未完成，因此 lifecycle 报告继续固定 `converter_available=false`、
`apply_available=false`、`verdict=blocked`，不能据此执行生产迁移或全新安装：

- MySQL/Percona/MariaDB source → PostgreSQL converter、copy journal、sequence reset 和逐值双向验证；
- target PostgreSQL/ClickHouse database 与最小权限 principal bootstrap、配置原子 promote、installation
  journal、最终切流和 rollback；
- provider 侧 Stripe 零状态、跨 datastore snapshot、完整 Redis ownership 与物理 topology 机器证明；
- ClickHouse retention apply、长期 archive/backup restore drill、aggregate projection 与目标容量基准；
- ClickHouse HA 或可演练的单节点重建、TTL/archive/restore drill，以及未来复制拓扑所需的 Keeper-backed
  schema 协调；当前固定单节点已有 schema migration 串行/崩溃恢复、installation binding 与 exact
  lineage readiness，但不得把这些保证外推到多节点；
- PostgreSQL outbox 分区/容量预算/磁盘告警/安全背压，以及启用任何 prune 前的第二恢复来源；
- API/worker 进程专属 0600 runtime config/secret mount；单一人工 manifest 不等于共享运行时 secret；
- native upgrade 当前 installation/build/schema epoch 的现场绑定、dry-run、impact estimator 和 apply。
