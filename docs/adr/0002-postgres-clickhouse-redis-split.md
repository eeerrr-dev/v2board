# 0002. PostgreSQL(业务)+ ClickHouse(可牺牲分析)+ Redis(运行态)三库分工

- 状态:已采纳(回填)
- 日期:2026-07(回填;决策早于此日期落地)

## 背景(Context)

旧站把一切压在 MySQL + Redis 上:业务事实、流量统计、Laravel queue/Horizon
混在一起,分析扫描和业务事务互相争抢,队列状态又散落在 Redis 里不可审计。
新版需要:

- 一个可以做 PITR、承载全部"丢了就完"数据的事务权威;
- 流量分析(节点上报、按日聚合)天然是 append-only 高写入量,放在事务库里
  迟早把业务拖垮;
- 会话、限流、租约这类短命状态需要低延迟,但绝不能变成账本;
- 单操作者运维,组件数量必须克制。

## 决策(Decision)

三库各司其职,边界写进不变量文档并由集成门禁看守:

- **PostgreSQL 18** 是唯一权威事务库:用户、订阅、订单、支付、余额、幂等状态、
  会话 epoch、operator config revision、migration ledger、installation identity,
  以及 typed **analytics outbox**(分析事件先在业务事务里落 PostgreSQL)。
- **ClickHouse 26.3 LTS** 只保存 PostgreSQL 先提交的不可变分析投影
  (`traffic.reported.v1` / `traffic.accounted.v1` 与批次日聚合),**明确可牺牲**:
  整库丢失只需空库重建 schema,继续投影未发布及新事件,绝不反写业务。
- **Redis 8.8** 专用 instance 的 `/0`,`noeviction`:只存会话查找、限流、租约、
  锁、心跳与短期缓存;API 与 worker 用不同 ACL principal,worker 无权访问认证
  状态。

分析投递用 **SQL outbox + relay**(`FOR UPDATE SKIP LOCKED`、不可变批次、整批
核对),配 normal/soft/hard 三级容量准入保护 PostgreSQL。

**明确拒绝的替代方案:**

- **单库全扛(PostgreSQL 兼做分析)** —— 无界分析扫描进入事务库被不变量文档
  明文禁止;OLAP 列存和 OLTP 的工作负载没有理由挤同一份磁盘预算。
- **消息队列中间件(Kafka/Redpanda/RabbitMQ)及 TimescaleDB/Elasticsearch/第二
  事务库** —— 全部列入禁止清单;只有容量门禁证明 SQL outbox 撑不住之后,消息
  流才允许作为独立架构变更提出。
- **把队列/账本放 Redis(旧 Horizon 模式)** —— Redis 不保存不可恢复账本,
  持久工作全部走 PostgreSQL。

## 后果(Consequences)

- 得到:备份策略极其清晰 —— 只有 PostgreSQL 需要 PITR,ClickHouse 全损接受,
  Redis 全损接受(全员重登);分析故障与业务解耦,ClickHouse 宕机时订单、
  认证、支付照常。
- 代价:要运维三个数据服务(版本钉死、TLS、ACL、日志轮转各一套);outbox 让
  PostgreSQL 兼职缓冲区,必须配容量准入、精确采样与告警,`hard_stop` 时流量
  结算会按设计 503。
- 代价:没有实时流处理与通用订阅能力;"已发布历史不承诺重放"意味着分析数据
  的完整性口径必须一直向操作者讲清楚,不能把陈旧投影伪装成权威数字。

## 证据

- [`../postgresql-clickhouse-invariants.md`](../postgresql-clickhouse-invariants.md)
  — 数据所有权表、outbox/admission 契约、禁止清单(§1、§2、§3)。
- [`../../backend/README.md`](../../backend/README.md) — Runtime architecture 与
  Analytics pipeline 章节。
- [`../../backend/rust/crates/analytics/src/lib.rs`](../../backend/rust/crates/analytics/src/lib.rs)
  — outbox/relay/admission 实现入口。
- [`../operations.md`](../operations.md) §2 — 按"丢了会怎样"分级的备份表。
- [`../../README.md`](../../README.md) — 三库固定版本与职责概述。
