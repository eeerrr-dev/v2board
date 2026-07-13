# PostgreSQL + ClickHouse 持久化不变量

状态：**native runtime 与分析投影已实现；`mysql-import.v1` executor 尚未接线。**

本文件取代“新版运行时以 MySQL 8.4 为目标”的设计。旧版数据库来源固定为
`references/wyx2685-v2board` 所代表的 Oracle MySQL 8.0/8.4；新版运行时不再把
MySQL 作为可选后端。

PostgreSQL native runtime 与 ClickHouse 投影已经接线。当前一次性工具只实现
`mysql-import.v1` validation 和 dump inspection；dump → staging → PostgreSQL 的生产 executor 尚未接线，
因此仓库还不能宣称可执行生产导入。

## 1. 固定产品选择

- 新版唯一权威事务库是 PostgreSQL 18，运行时接受当前 18.x 安全补丁版，不自动跨
  PostgreSQL major 升级。
- 新版分析库是 ClickHouse 26.3 LTS，运行时接受仍获安全更新的 26.3.x 补丁版，不跟随
  monthly innovation release 自动升级。
- Redis 只保存 session、限流、lease、锁和短期缓存，不保存不可恢复账本。
- 不引入 TimescaleDB、Kafka、Redpanda、RabbitMQ、Elasticsearch/OpenSearch 或第二套
  事务数据库。只有经过容量门禁证明 SQL outbox 无法满足要求后，消息流才可作为独立
  架构变更提出。
- MySQL importer 只读取完整 dump 导入的一次性 staging MySQL 8.0/8.4 engine；不连接或修改
  live 旧 MySQL。临时 engine 可位于迁移/新主机的一次性容器、临时 VM 或其他受控私网主机，完成
  后删除；新生产 runtime 不安装或保留 MySQL 服务。MySQL 5.7、Percona、MariaDB 和兼容代理全部
  拒绝。source patch version 不改变 PostgreSQL target。

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
| schema migration ledger、installation identity、config revision | PostgreSQL | 否 |

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
事件；ClickHouse 不可用时已提交的 outbox 必须保留并重试。新流量事务在下述 normal/soft 容量窗口内
仍可提交；达到 hard-stop 时只回滚会新增 analytics event 的流量事务，不能继续无界占用 PostgreSQL。

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

### 3.1 PostgreSQL outbox 容量准入

初始 target 的 `mysql-import.v1` 必须填写 `analytics_admission`，导入阶段把它安装为与
installation UUID 绑定、runtime 不可修改的单例 policy。它同时冻结三组带 hysteresis 的阈值：pending
rows、outbox relation bytes、最老 pending age；以及 dedicated PostgreSQL database 的容量、hard/soft/
recovery 最小余量、每 event 预留字节、soft 每秒新增行上限、采样周期和 stale deadline。
`capacity_evidence` 必须指向该 PostgreSQL database 所在独占 volume/quota 的现场证据；可用余量固定按
`database_capacity_bytes - pg_database_size(current_database())` 计算，不能把共享磁盘总容量或随手估值
冒充机器绑定预算。

独立 admission worker 周期性在 PostgreSQL 18 内取得精确值：pending `COUNT/MIN(created_at)`、outbox
主 relation（含 fork）、主 indexes、TOAST relation、`pg_total_relation_size` 和 `pg_database_size`。
生产者不做这些扫描；两个真实生产者都在原业务事务中锁定轻量 state singleton，并只按
`INSERT ... ON CONFLICT` 实际新增的 event 数预留容量：

- `normal`：阈值以下正常接收；
- `soft_pressure`：记录告警，并用串行的一秒窗口限制新 event；超额 API 事务返回 HTTP 429，outbox/
  business mutation 一起回滚；
- `hard_stop`：超过 hard rows/bytes/age/headroom，或精确样本超过 stale deadline 时，server traffic API
  在提交前返回 HTTP 503，accounting worker 保留 report 为可重试；认证、订单、支付不经过该 gate；
- relay 永远继续 claim/projection；published/quarantined 在同一 PostgreSQL 事务释放 reservation。精确
  采样达到全部 recovery 水位后才自动从 soft/hard 回到 normal，避免水位附近抖动。

`/readyz.checks.analytics_admission` 暴露 policy hash、状态、sample freshness、精确/已预留 rows、最老
age、heap/index/TOAST/total bytes、database bytes 和 capacity headroom；它不会把整个 API 摘流，只用
`traffic_writes_available` 表达流量入口是否可写。worker 同步写入有界 Redis hash
`RUST_ANALYTICS_ADMISSION`，并在状态变化时输出结构化 warn/error/recovery 日志。
精确 `COUNT` 不是每个请求的热路径；生产清单示例使用 5 秒采样、30 秒 stale deadline，操作者必须按真实
outbox 索引规模做基准后填写，不能为了更漂亮的 dashboard 把百万行精确扫描固定成每秒一次。

在长期 archive、generation replay 与 restore drill 完成前，PostgreSQL 的 terminal published row
不得由 runtime 自动清理；pending 与 quarantined 更不得删除。显式 prune API 只供隔离测试，不能进入
生产调度。未来启用任何 replay window 都属于破坏性变更，必须先证明第二份恢复来源、容量预算和恢复
时间，并展示将失去的永久 event-id 冲突证据。

不得把 ClickHouse `ReplacingMergeTree` 后台 merge 当作计费幂等。生产 raw 表使用普通
append-only MergeTree/ReplicatedMergeTree；relay 对不确定 acknowledgement 必须按 batch
manifest 查询并核对全部 event id/content hash。部分写入或冲突属于完整性事故，需要隔离并
重建目标月 generation，不能补几行后假装成功。产品权威数字仍从 PostgreSQL 读取。

## 4. PostgreSQL schema 方向

- PostgreSQL 使用独立于旧 MySQL 的首发前 SQLx baseline。旧 43 个 MySQL migration 的版本和
  checksum 不会写入 PostgreSQL ledger。
- native 尚未发布，当前 SQLx 和 ClickHouse migration 可以直接整理成一个干净初始基线；
  旧 MySQL 8 数据只由 `mysql-import.v1` converter 转换。
- 所有高增长 identity、用户和关联外键采用 `BIGINT`；金额继续使用整数 cents，并有
  非负/范围 check。
- 状态枚举使用 `SMALLINT` + check，真正二值字段使用 `BOOLEAN`。
- 管理后台可变配置以 PostgreSQL 不可变 revision + 单例 active pointer 为唯一权威源；公开设置使用
  `JSONB`，四个 operator secret 使用 AES-256-GCM 密文。revision 只 INSERT，active pointer 以
  expected-revision CAS 单调推进；API/Worker 各自记录 applied/rejected ack。可查询或参与约束的关键字段
  保持普通列。
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
- 当前 reported/accounted 各有独立的按批次日聚合表，reported 与真正 applied charged 指标不得混算。
  relay 对 raw 与日聚合分别使用稳定 token 并分别按 `ingest_batch_id` 全值核对；26.3 实测证明 dependent
  materialized view 会在原始表已去重的并发重试下重复累计，因此不能承担当前精确聚合。未来小时、节点、
  套餐、费率或排行聚合必须另立版本化契约和同等级幂等证明。
- 原始事实 TTL 和聚合 TTL 分开配置。降低 retention 属于破坏性配置更新，必须先展示将
  删除的分区范围并二次确认。
- ClickHouse migration 有独立 schema ledger；不能借用 PostgreSQL `_sqlx_migrations`。
- schema 演进采用 additive column / dual projection / backfill / verified swap / delayed drop，
  不能原地依赖大规模 mutation。
- ClickHouse 专用 analytics reader、outbox writer 和 schema migrator 使用不同的最小权限 principal；
  relay writer 仅有 raw/按批次日聚合 INSERT、核对自身 immutable batch 所需的受限 SELECT，以及读取
  schema/installation/retention binding 的 readiness SELECT，不拥有 DDL。
  schema principal 只具有目标表 DDL、schema 校验所需 system metadata SELECT，以及 migration ledger
  的 SELECT/INSERT；不能误写成不可能运行 migrator 的“纯 DDL-only”。
  当前 API 不查询 ClickHouse，因此 reader 不物化到 API/worker runtime；bootstrap/schema 也只供
  一次性 schema job。
- PostgreSQL outbox 当前保留完整 replay history；只有经过恢复演练的 ClickHouse replicated backup
  或不可变对象归档成为第二恢复来源后，才允许提出有界 replay window。若 raw event 被定义为长期
  审计证据，还必须写不可变对象归档。不能在没有第二恢复来源时清理权威重放记录。

## 6. 连接和权限

目标 PostgreSQL 至少区分：

- bootstrap principal：只用于创建 database/roles，完成后撤离；
- migration principal：拥有 schema DDL 和 migration ledger；
- API principal：只拥有 API 所需 DML；
- worker principal：拥有 worker、queue 和 outbox 所需 DML。

operator-config 表在普通业务 DML allowlist 之外使用不对称权限：API 可 SELECT/INSERT revision、
SELECT/INSERT/UPDATE active state 与 API ack；Worker 对 revision/state 只读，只能写 Worker ack；双方只读
对方 ack。revision identity sequence 仅 API 有 USAGE，双方都没有 config 表的 DELETE/TRUNCATE/REFERENCES/
TRIGGER 权限。角色 `0600` 文件只含 boot 凭据，不含首次 baseline，也不是长期动态配置权威源。

PostgreSQL role 没有 MySQL `'user'@'host'` 语义。客户端来源限制由 `pg_hba.conf`、托管网络
ACL、安全组或私网控制；provision spec 必须记录该外部控制的声明和证据，不能假称 SQL
`CREATE ROLE` 已限制来源。

目标 ClickHouse 至少区分 bootstrap、schema migrator、outbox writer 和预留给专用分析消费者的
analytics reader；当前常驻进程只有 worker 得到 writer。
所有生产 PostgreSQL、ClickHouse 和 Redis 连接都必须使用经过主机身份验证的 TLS 或在
受控私网加密隧道内；开发环境例外只能存在于 Docker 内部网络。

## 7. 初始数据库与 MySQL 导入

首发前只有 `mysql-import.v1` 创建初始业务数据：

1. 操作者停止旧 API、worker、scheduler、支付入口和远端节点报告者；
2. 从 Oracle MySQL 8 导出全部业务表和行，记录 dump SHA-256，旧数据库保持不变；
3. 把 dump 导入一次性 staging MySQL 8 engine，且不运行旧 trigger、routine 或 event；该 engine
   可以位于独立迁移主机，完成后删除，不是新服务器的长期依赖；
4. 在全新的 PostgreSQL 18 target 上运行当前 baseline，并转换固定保留行；
5. ClickHouse 从空 native event history 开始，新 Redis 从空状态开始；
6. 从 `target` 和 `runtime` 生成 API/worker 两份 `file_only + boot_only` 配置；
7. 验证 schema、行数、键、关系、关键值、sequence、固定丢弃结果、最小权限和配置 parser，
   然后才能启动 API/worker。

旧 MySQL 不是 target，也不被 converter 修改。staging 只是处理 dump 的临时输入。导入失败时丢弃
staging 和不完整的新 target，修正问题，再从 dump 向新的空 target 完整运行一次；不保存中间状态，
也不引入在线迁移或新旧双写。

表名边界固定为：旧 MySQL source 使用 dump 中真实的 `v2_*` 名称；新 PostgreSQL/ClickHouse target
不带该前缀，用户与订单 target 分别使用 `users`、`orders` 避免 PostgreSQL 关键字。没有 prefixed
native schema，也不为未发布名称建立 rename migration 或兼容 view。

固定保留/丢弃边界如下：

- source `v2_user` 的 ID、password hash、永久 token、balance、MySQL 已落盘 `u/d/transfer_enable`
  等业务值原样写入 target `users`；不会因任何丢弃项退款、补偿或重算余额；
- source `v2_stat` 写入 target `stat`；source `v2_stat_server`、`v2_stat_user`、`v2_log`、
  `v2_mail_log` 不进入 target；
- 旧节点、route 与 credential 全部丢弃，target 节点从空开始，由操作者使用新 token 手工重建；
- 旧 Redis 完全不检查、不导入；未落 MySQL 的尾流量、queue/retryable failed work、session、OTP、临时
  订阅 URL、cache/lock/lease/rate-limit 与 Horizon metadata 全部丢弃；MySQL `failed_jobs` 同样丢弃；
- 大小写不敏感地以 `stripe` 开头的 payment 配置及 status 0/1 Stripe 订单丢弃；工具不访问 Stripe，
  status 2/3/4 Stripe 订单保留业务历史，但 `payment_id`、`callback_no` 和派生 callback hash 为空；
- 非 Stripe payment 配置和非 Stripe 未完成订单按普通业务数据保留；
- ClickHouse 只接收新版 PostgreSQL outbox 的新事件，任何旧统计都不展开、估算或伪造成 raw event。

当前只实现 `mysql-import.v1` validation 和只读 dump inspection；生产 converter executor 尚未接线。
完整规则见[MySQL 一次性导入不可变契约](mysql-import-invariants.md)。

## 8. 故障和健康语义

- PostgreSQL 不可用：核心 API/worker 不 ready，禁止以 role file、Redis 或 ClickHouse 代替权威读写；
  已运行进程可保留最后已验证的 operator snapshot 处理明确允许的短暂重连窗口，但不能应用未知 revision，
  Worker 启动时没有 active authority 一律失败关闭。
- Redis 不可用：依赖 session/lease 的功能按既有 fail-closed 契约降级。
- ClickHouse 短期不可用：订单、认证、支付继续；normal/soft 窗口中的流量结算继续，analytics 标记
  stale/unavailable，outbox 积压并告警，禁止回退为对 PostgreSQL 的无界分析扫描。长期故障达到预先
  冻结的 PG rows/bytes/age/headroom hard 水位后，只有新增 analytics 的流量事务 503/重试；relay 继续
  排空，恢复到全部 recovery 水位后自动开放。
- 本地容器 stdout/stderr 与数据库内部日志必须同时有界轮转；Compose 固定为 `local` driver、
  `10m × 3`。裸机生产使用有界 journald 和各数据库原生日志轮转，日志耗尽共享磁盘按
  PostgreSQL/ClickHouse 不可用告警。
- analytics 响应必须携带 freshness/lag；不得把陈旧投影伪装成实时权威数据。
- outbox oldest age、pending rows/bytes、publish rate、retry count、ClickHouse parts/merge、
  PostgreSQL WAL/vacuum/lock、replica lag 和 restore time 都是发布门禁指标。

## 9. 实现状态与剩余门禁

已完成并由代码门禁覆盖：

- native Rust runtime、SQLx concrete types、SQL 和 final-state baseline 全量切换 PostgreSQL；
- PostgreSQL 首发前 baseline、374 条 live SQL prepare、并发业务 invariant 与 worker reconciliation；
- typed analytics outbox、immutable batch、ClickHouse 26.3 version/schema gate、lost-ACK retry、完整核对、
  quarantine、默认不清理的 replay history 和真实 PostgreSQL→ClickHouse 往返；
- installation-bound outbox admission policy、精确 PostgreSQL relation/index/TOAST/database 测量、
  normal/soft/hard hysteresis、并发 pre-commit fail-closed、独立 relay drain 与自动恢复；
- Docker 本地 PostgreSQL/ClickHouse/Redis、固定 image digest 和隔离 integration target；
- strict `mysql-import.v1` manifest、dump SHA-256 inspection、固定 converter registry 和固定
  retained/discarded row policy。

尚未完成的是 Oracle MySQL 8 dump → staging MySQL → PostgreSQL conversion executor、固定结果的
真实数据验证、配置安装和 native Linux/systemd 启动验收。在这些端到端能力完成前，仓库只支持
`validate` 和只读 `inspect`，不能宣称能够执行生产导入。

`mysql-import.v1` 不迁移或激活仓库外的 V2bX/XrayR 等节点程序：旧节点不被 importer 审计或复制，
target node inventory 为空，操作者在导入后以新 token 手工重建。

仍需作为长期容量/运维工作持续处理、但不伪装成这次空 ClickHouse 初始迁移 blocker 的项目包括：

- PostgreSQL outbox 的未来分区策略、外部磁盘/WAL 告警，以及启用任何 prune 前的第二恢复来源；准入
  预算和应用内安全背压已经实现，但不能替代基础设施监控或 replay archive；
- ClickHouse 更多产品聚合、目标容量基准和备份演练；当前单节点是明确部署策略，HA/Keeper 属于可用性
  扩展，不能把已有 standalone 证明外推到多节点。
