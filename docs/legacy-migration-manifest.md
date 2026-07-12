# 旧版迁移清单 v3

本指南只适用于 `references/wyx2685-v2board` 固定 commit
`7e77de9f4873b317157490529f7be7d6f8a62421` 到新版 PostgreSQL 18 +
ClickHouse 26.3 + Redis 架构的迁移。旧 MySQL、Percona 或 MariaDB 只可能是只读来源，绝不是新版
runtime 的可选数据库。

完整的数据所有权、outbox 和故障语义见
[PostgreSQL + ClickHouse 持久化不变量](postgresql-clickhouse-invariants.md)，三类 lifecycle 的共同
契约见[安装、迁移与升级不可变契约](upgrade-invariants.md)。

## 为什么使用一个手填文件

手填一个严格 JSON 文件比猜测和转换旧配置更容易复查，但它只负责表达**目标意图**：

- 旧 MySQL/MariaDB 与两个旧 Redis 逻辑库的只读连接；
- PostgreSQL bootstrap/migration/API/worker 四个 principal 的连接；
- ClickHouse bootstrap/schema/writer/reader 四个 principal、retention 和网络证据；
- 空 target Redis、API/worker 两个最终 runtime 路径、公共行为配置和明确的迁移决策。

工具不会导入、合并、执行或推断旧 `.env`、`config/v2board.php`、theme、PHP、CSS 或 JavaScript。
`runtime` 的每个目标值都由操作者明确填写；数据库、Redis、文件和现场版本等可探测事实仍必须自动
检查，不能靠清单中的自报值绕过。

目标 datastore 连接由清单分别物化到 API/worker file-only map；当前 CLI 只验证这两个 map，不写
文件。bootstrap 和 schema/migration 凭据只供 lifecycle job 使用，不得进入长期 API/worker runtime。
API map 不含 worker URL 或任何 ClickHouse 凭据；worker map 不含 API URL、reader 或 DDL 凭据。

清单 schema 固定为 `schema_version: 3`。v2 已退役并被 loader 明确拒绝，不能把旧 v2 文件改个版本号
继续使用。

## 固定选择

`legacy_reference_migration` 不提供自由组合；以下值必须原样出现：

| 项目 | 固定值 | 结果 |
| --- | --- | --- |
| 旧配置 | `manual_only` | 目标值全部手填，不从旧配置导入 |
| 登录态 | `logout_all` | 不转换 Laravel session；用户和管理员切换后重新登录 |
| 旧 cache | `discard_ephemeral_after_fence` | 只在 producer 已 fence 后放弃已分类的临时 cache |
| Stripe | `assert_none` | 自动检查 Stripe 相关 inventory 为零，否则阻断 |
| 临时订阅 token | `assert_none` | 等待所有已签发 OTP/TOTP 窗口耗尽，否则阻断 |
| 节点 | `one_shot_offline_cutover` | 全部 reporter 停机时批量换 scoped token/幂等键，全部验证后统一启动 |
| 旧 theme | `discard_confirmed` | 明确接受旧视觉/脚本资产不进入新版 |
| custom rules | `none` 或 `discard_confirmed` | 不盲拷旧模板；存在未处置文件时阻断 |

同时固定：

- `runtime.configuration_source=file_only`；
- `target.api_runtime_config_path=/var/lib/v2board/api/config.json`、
  `target.worker_runtime_config_path=/var/lib/v2board/worker/config.json`；
- native runtime 不含 query/form auth fallback、旧 JWT decoder/cutoff 或全局 node-token fallback；
- `runtime.server_require_idempotency_key=true`；
- `APP_KEY`、server token、lifecycle audit key 和所有 target 密码互不复用。

## 永久订阅 token 与旧 Redis

旧用户的永久订阅凭据是 source SQL 表中的 `v2_user.token`。它与用户 ID、`uuid`、密码 hash、余额、
套餐和流量一样属于必须原值转换到 PostgreSQL 的业务数据。

Redis 中的 `otp_`、`otpn_`、`totp_` 是短期订阅映射或验证 cache，不是永久 token：

- mode 1 通常先写双向 mapping，再签发临时 URL；
- mode 2 可由用户 ID、永久 token 和时间窗计算 URL，签发时完全不写 Redis；首次验证时才可能写 cache。

因此“Redis 扫不到 key”不等于没有已签发链接。非 mode 0 的来源必须记录停止签发时间，并等待完整
有效期和时钟裕量；最终 `plan` 还会要求相关 key 为零。

另一类 Redis 数据绝不是 cache：`v2board_upload_traffic` 和
`v2board_download_traffic` 保存节点已上报、但尚未计入 SQL `v2_user.u/d` 与统计表的增量。queue 和
retryable failed work 也可能仍含待执行的业务动作。它们必须在 fence 后以耐久、可恢复的方式排空并
与 SQL 对账，不能直接丢弃或 `FLUSHDB`。

## 为什么 source 要填两个 Redis URL

固定旧版通常把直接 Redis、queue 和流量 hash 放在 `REDIS_DB`，把 Laravel `Cache::`、OTP/TOTP 等放在
`REDIS_CACHE_DB`。两者还分别受 `REDIS_PREFIX` 和 `CACHE_PREFIX` 影响，所以清单必须提供：

- `redis_default_url`；
- `redis_cache_url`；
- `redis_connection_prefix`；
- `redis_cache_prefix`。

只填一个 URL 可能产生危险的假零结果。v3 目前只接受 `legacy_cache_driver=redis`；真实来源若使用其他
driver，必须先增加对应的只读 inventory adapter，不能伪填 Redis。

## Target 由 lifecycle 创建

操作者提供已存在的 bootstrap principal 连接和外部网络控制证据，不手工预建目标业务库或长期
principal。future `apply` 必须在操作者根据在线 `inspect` 作出一次不可逆迁移授权后，先 fence 并执行
内部 final recheck，再写 durable pending journal 和创建：

### PostgreSQL 18

- `bootstrap_database_url`：连接已经存在的系统库，principal 只用于创建 target database/roles；
- `migration_database_url`：target database 上的 DDL 与 migration ledger principal；
- `api_database_url`：只拥有 API 所需 DML；
- `worker_database_url`：只拥有 worker、queue 和 outbox 所需 DML。

四个 URL 必须指向同一 PostgreSQL 18 实例；后三个指向同一 target database，principal 彼此独立，并在
生产使用 `sslmode=verify-full`。目标库及 migration/API/worker roles 必须不存在；bootstrap role 已由
基础设施提供且具备受审计的创建权限。collation/ctype 固定为 `C.UTF-8`。
PostgreSQL role 不携带 MySQL 的 `'user'@'host'` 范围，所以清单必须记录外部 `pg_hba.conf` 与网络策略
证据。

### ClickHouse 26.3 LTS

清单分别填写 bootstrap、schema、仅允许 raw INSERT 与批次核对 SELECT 的 relay writer，以及面向分析查询的
select-only reader principal。bootstrap
principal 已由基础设施提供；目标 database 及 schema/writer/reader principals 必须不存在。初始拓扑
固定为 standalone non-replicated，raw/aggregate retention 必须显式填写并满足 `aggregate >= raw`。

ClickHouse 只保存由 PostgreSQL outbox 投影、可重建的分析事实；短暂不可用时不阻止认证、订单、支付
或流量结算的同步事务。当前 API 不消费 reader；worker 只得到 writer。长期不可用会耗尽 PostgreSQL
outbox 容量，容量/磁盘水位/安全背压门禁尚未实现，因此仍是 production apply blocker。API/worker
禁止同步双写 PostgreSQL 和 ClickHouse。

### Redis

目标 URL 必须使用 `rediss://`，与两个 source Redis identity 不同，并且选定逻辑 DB/namespace 为空。
Redis 没有“创建逻辑 DB”步骤；工具只验证空状态，绝不会用 `FLUSHDB` 帮忙变空。

## 先检查，再计划，最后才可能写入

固定顺序是：

1. `validate` 离线校验 v3 文件、全部必填 key/type、URL、secret 独立性和固定决策。
2. 旧系统仍在服务时运行 `inspect`，在线只读检查来源、target 和当前 blocker。它不授权进入迁移。
3. 工具输出不可变的检查报告；操作者查看后只作一次决定：是否针对精确
   `operation_id + inspect report_sha256` 启动不可逆的 one-shot `apply`。拒绝或不确认都不进入维护、
   不停止旧服务，也不创建 target。
4. future `apply` 在同一次调用中先 fence 旧 API writer、worker、scheduler、全部 node reporter 和临时
   链接签发，再排空并对账流量、queue、failed/paid-pending work，建立一致 backup 并验证隔离恢复。
5. 同一 `apply` 在首次 mutation 前自动执行 final recheck；任何 manifest、现场 identity、数据或报告
   发生变化都立即中止，不插入第二次人工等待。
6. final recheck 通过后，同一 `apply` 才写 pending journal、创建 target、bulk 转换、逐值验证、物化
   配置、离线批量更新全部节点，并只启动新版一次；验收成功后立即完成 source retirement。

`apply` 不得实现 CDC、双写、shadow read、跨维护窗口 backfill 或按节点逐批恢复服务。journal/checkpoint
只用于同一次停机操作的崩溃恢复。最终提交后统一启动新版，随即撤销 source 账号、隔离网络、永久停服
MySQL/MariaDB 与旧 Redis，并从服务器删除一次性 lifecycle/MySQL 工具；只保留带 SHA-256 的加密冷归档。
此后禁止 MySQL runtime rollback，只能恢复 PostgreSQL/ClickHouse 或 forward recovery。

当前仓库只实现 `validate`、`inspect` 和 `plan`，且 converter/apply 仍不可用。报告固定
`converter_available=false`、`apply_available=false`、`verdict=blocked`、
`next_action=resolve_blockers`；不存在 lifecycle `apply` 命令。即使某些现场检查全部通过，也不能手工把
它解释成受支持迁移许可。

## 使用 v3 示例

模板含公开 placeholder，不能直接运行。复制到仓库外，逐项复查并填写，生成新的 operation UUID，
限制为 owner-only 权限：

```bash
cp docs/examples/legacy-migration.v3.example.json /secure/private/legacy-migration.json
chmod 600 /secure/private/legacy-migration.json

v2board-lifecycle validate --manifest /secure/private/legacy-migration.json
v2board-lifecycle inspect --manifest /secure/private/legacy-migration.json

# 当前仅供开发审计；future apply 会在维护窗口内部自动执行同一 final recheck：
v2board-lifecycle plan --manifest /secure/private/legacy-migration.json
```

清单必须是 regular、non-symlink 文件，大小为 1 byte 到 1 MiB，且在 Unix 上不得授予 group/world 权限。
datastore URL 密码中的特殊字符要 percent-encode。生产 target 必须使用经过身份验证的 TLS；source 只有
在具名可信维护网络或加密隧道内才可声明非 TLS 例外。

报告不会输出明文 secret。`manifest_binding_hmac_sha256` 绑定原始清单 bytes；
`report_binding_hmac_sha256` 与 `report_sha256` 绑定检查结果和现场 identity。修改 JSON 空格、凭据、
endpoint、backup reference 或 runtime 值都会改变 binding。

## 当前只读检查覆盖与缺口

legacy `inspect/plan` 已检查或报告：

- source MySQL/Percona 5.7+ 或 MariaDB 10.2+ 的实际 vendor/version、read-only session、结构指纹、
  standalone 可见拓扑和 core object inventory；
- 未完成/paid-pending order、Stripe 零状态、关系/唯一性/collation 冲突、giftcard redemption；
- 节点 `group_id`、coupon `limit_plan_ids`、order `surplus_order_ids`、giftcard `used_user_ids` 的 JSON ID
  array 类型、正整数/目标范围、引用完整性和数字字符串 normalization 计数；
- 两个 source Redis 的流量、queue、failed work、reset lock、OTP/TOTP 窗口与实例 identity；
- target PostgreSQL 18 能力、库/角色为空、collation 与外部控制声明；
- target ClickHouse 26.3 LTS、database/principal 为空、standalone 状态、grant/retention 声明；
- target Redis 为空、版本/命令能力与 source 隔离。

仍未实现并持续 fail closed 的核心能力包括：完整来源 lineage/config/artifact 证明、MySQL/MariaDB 到
PostgreSQL 一次性 offline converter、跨 datastore snapshot 一致性、完整 Redis ownership classifier、provider 侧
Stripe 零状态、target bootstrap、operation journal、数据逐值验证、ClickHouse 初始投影、配置原子
promote、backup binding、pre-commit abort/restore、离线节点批量验收、source retirement 和最终 `apply`。
ClickHouse 固定单节点的 schema lock、
崩溃恢复、精确 lineage 与 installation binding 已实现；TTL/archive/restore drill、HA/Keeper 与跨节点
schema 协调，以及 PostgreSQL outbox 容量预算/磁盘水位/安全背压仍未完成；secret split 通过也不会
解除这些 blocker。

普通 `v2board-api migrate` 只运行已确认 native PostgreSQL lineage 的 SQLx migration；它不是 legacy
adoption/converter 命令，绝不能指向旧 MySQL/MariaDB 或未知 PostgreSQL 数据库。
