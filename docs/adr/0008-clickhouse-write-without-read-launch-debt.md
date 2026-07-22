# 0008. 首发接受 ClickHouse 只写不读:analytics reader 无生产调用方

- 状态:已采纳(记录已知技术债)
- 日期:2026-07-21

## 背景(Context)

写路径已经完整落地并长期在生产运行:API 在同一事务里写入类型化不可变
`traffic.reported.v1` / `traffic.accounted.v1` 事件到 PostgreSQL outbox
(`crates/analytics/src/event.rs`、`outbox.rs`),worker 的 relay 把它们
INSERT 进 ClickHouse MergeTree 并做整批核对(`crates/workers/src/analytics.rs`、
`crates/analytics/src/projection.rs`),独立的 admission 循环持续采样
outbox 压力(`crates/analytics/src/admission.rs`)。压力进入 `hard_stop` 时,
流量结算 API 会**返回 503**(`crates/api/src/server_api/traffic.rs`)——这是
一条真实、可观测、会影响用户可见行为的生产路径。

与此同时,读路径只有一个函数:
`crates/analytics/src/reader.rs::read_applied_daily_traffic`,通过
`crates/analytics/src/lib.rs` 对外 `pub use`。检查其调用方:

```
$ grep -rn "read_applied_daily_traffic" backend/rust
backend/rust/crates/analytics/tests/clickhouse_roundtrip.rs:10
backend/rust/crates/analytics/tests/clickhouse_roundtrip.rs:511
backend/rust/crates/analytics/tests/clickhouse_roundtrip.rs:527
backend/rust/crates/analytics/src/lib.rs:34   (仅 re-export)
backend/rust/crates/analytics/src/reader.rs:50 (定义 + 自身单测)
```

除测试与自身 re-export 外,`api`、`application`、`workers` 三个消费方
crate 里没有任何一处调用它。也就是说:ClickHouse 里持续产生并校验通过的
批次聚合数据,当前**没有任何生产代码读取**——没有面向用户的用量图表,没有
管理端分析视图,没有告警/运营脚本消费它。`docs/architecture.md`(§"事件流"
一节)已经如实写着"API 当前不查询 ClickHouse",本 ADR 把这一状态明确记录为
*有意接受的阶段性技术债*,而不是遗漏。

## 决策(Decision)

首发阶段**保持写路径全量生产运行、读路径暂不接入任何调用方**:

- ClickHouse outbox 写入 + admission 准入(含其 `hard_stop → 503` 背压行为)
  是**当前的生产契约**,不因为读侧空缺而降级或关闭;流量结算在高压下拒绝
  写入优先于无界积压或丢数据,这个权衡独立于"谁读它"成立。
- `reader.rs` 保留在 crate 里,由 `clickhouse_roundtrip.rs` 集成测试和自身
  单测持续验证其查询语义(installation 绑定校验、schema-major 隔离、日期
  范围校验)与 schema 演进保持同步,防止它在未来被真正接线时才发现已经腐化。
- 不在本批引入任何新的 API 路由、管理端页面或后台任务去消费它——那是一个
  独立的、有自己产品/UX/权限设计空间的项目,不应该被塞进一次审计修复里
  仓促拼凑。

**接受"写而不读"的理由:**

- 写路径的价值不依赖读路径存在:它已经在为未来的分析能力积累经过校验的
  历史数据(ClickHouse 允许全丢,但只要在线就应该持续正确地积累),延后
  接入读端不损失已产生的数据。
- 读端不是一个把现成函数接到路由上的小改动:真正有用的读侧需要回答"谁能
  读谁的数据"(RBAC/用户隔离)、"结果怎么用"(用户可见的用量图表？管理端
  报表？运营告警？)、"新增只读 API 契约"(路由、payload、i18n、golden
  lane)、以及可能的聚合/缓存策略,这些都需要独立的产品决策,不应该在
  一次架构审计的修复批次里为了"消灭零调用方警告"而临时拍板。
- 强行现在删除 reader.rs 同样是错误方向:schema 与查询逻辑已经写好并有
  测试覆盖,推倒重写不会比"保留 + 记录债务"更省成本,反而会在未来真正
  需要读端时丢失已验证的实现基线。

**明确拒绝的替代方案:**

- **现在就接一个最小可用的读端(哪怕只是一个内部 debug 路由)** ——
  没有权限模型、没有产品位置的"内部路由"最终会被误用为事实上的公开
  契约,或者半途而废地留下另一处未完工代码;比"零调用方"更差。
- **删除 reader.rs 与其测试,等真正需要时再重写** —— 丢弃已验证的查询
  实现(installation 绑定、schema-major 隔离、日期范围校验)只为消除一个
  静态告警,是用未来的重复劳动换现在的表面整洁。
- **降级或关闭 admission/outbox 写路径,直到有读端再启用** —— 写路径的
  背压行为(`hard_stop → 503`)保护的是 PostgreSQL 与 ClickHouse 本身的
  容量安全,与"是否有人读"无关;关闭它不会让技术债消失,只会让容量风险
  失去唯一的现有防线。

## 后果(Consequences)

- 得到:如实记录当前状态,后续审计或新贡献者不会把"reader.rs 零调用方"
  误判为遗漏的接线或死代码可以直接删除;保留了已验证的读取实现作为未来
  接入的起点。
- 代价:`read_applied_daily_traffic` 在生产 binary 里当前是仅被测试引用的
  死代码;`cargo` 层面它不会触发 `dead_code`(因为 `pub use` 对外导出),
  但这是一个需要长期口头/文档维护的"已知例外",而不是编译器强制的不变量。
- 代价:ClickHouse 里积累的历史数据在读端接入前对产品没有任何可见价值;
  如果读端长期不落地,写路径的存储与 admission 运维成本(参见
  `docs/postgresql-clickhouse-invariants.md`)就是纯沉没成本。
- 后续触发条件(用于判断"该接入读端了"而不是让这条债务无限期存在):
  出现第一个真实消费需求(用户用量图表、管理端分析视图,或运营告警脚本)
  时,必须先补齐该消费方的权限模型与只读 API 契约(路由、payload、golden
  lane、i18n),再把 `read_applied_daily_traffic` 接进去;接入的同一提交
  必须把本 ADR 状态更新为"已取代"并链接后继 ADR,而不是静默开始调用它。

## 证据

- `backend/rust/crates/analytics/src/reader.rs` — 唯一的读函数
  `read_applied_daily_traffic`,当前仅被
  `backend/rust/crates/analytics/tests/clickhouse_roundtrip.rs` 与自身单测
  调用;`grep -rn "read_applied_daily_traffic" backend/rust` 在 `api`、
  `application`、`workers` 三个消费方 crate 中零命中。
- `backend/rust/crates/analytics/src/admission.rs`、
  `backend/rust/crates/analytics/src/outbox.rs`、
  `backend/rust/crates/workers/src/analytics.rs` — 全量生产运行的写路径:
  admission 准入循环、outbox 入队/relay、批次投影校验。
- `backend/rust/crates/api/src/server_api/traffic.rs` — 流量结算在
  `hard_stop` 准入状态下返回 `503 SERVICE_UNAVAILABLE` 的生产行为。
- [`../architecture.md`](../architecture.md) §"事件流"("API 当前不查询
  ClickHouse;只有 worker 的 analytics relay 以最小权限 writer 写入")与
  容量准入一节。
- [`../postgresql-clickhouse-invariants.md`](../postgresql-clickhouse-invariants.md)
  — outbox/admission/投影不变量,写路径运维契约的权威来源。
- [`0002-postgres-clickhouse-redis-split.md`](0002-postgres-clickhouse-redis-split.md)
  — ClickHouse 作为"可牺牲分析投影"的三库分工决策,本 ADR 是它在读侧的
  首发范围补充说明。
