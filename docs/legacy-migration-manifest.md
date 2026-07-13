# 旧版迁移清单 v4

本指南只适用于 `references/wyx2685-v2board` 固定 commit
`7e77de9f4873b317157490529f7be7d6f8a62421` 到新版 PostgreSQL 18 +
ClickHouse 26.3 + Redis 架构的迁移。唯一允许的数据库来源是 Oracle MySQL 8.0/8.4；MySQL 5.7、
Percona、MariaDB 和兼容代理均会被拒绝。MySQL 绝不是新版 runtime 的可选数据库。

完整的数据所有权、outbox 和故障语义见
[PostgreSQL + ClickHouse 持久化不变量](postgresql-clickhouse-invariants.md)，三类 lifecycle 的共同
契约见[安装、迁移与升级不可变契约](upgrade-invariants.md)。

## 为什么使用一个手填文件

手填一个严格 JSON 文件比猜测和转换旧配置更容易复查。v4 是唯一的迁移输入，同时表达**目标意图**和
**裸机执行意图**：

- 旧 Oracle MySQL 8 与两个旧 Redis 逻辑库的只读连接；
- PostgreSQL bootstrap/migration/API/worker 四个 principal 的连接；
- ClickHouse bootstrap/schema/writer/reader 四个 principal、retention 和网络证据；
- 空 target Redis、公共行为配置和明确的迁移决策；API/worker 两个最终 runtime 路径由 v4 固定派生，
  不要求操作者重复填写。
- 不可变 release 的 ID/digest、旧 systemd 单元与备份/隔离恢复输入。journal、receipt、
  release archive、runtime secret、staging 和 retirement state 等内部路径由 `operation_id` 唯一生成，不再手填。

工具不会导入、合并、执行或推断旧 `.env`、`config/v2board.php`、theme、PHP、CSS 或 JavaScript。
`runtime` 的每个目标值都由操作者明确填写；数据库、Redis、文件和现场版本等可探测事实仍必须自动
检查，不能靠清单中的自报值绕过。

目标 datastore 连接由清单分别物化到 API/worker file-only map；`validate/inspect` 只验证这两个 map，
已接线的 legacy executor 仅在 journaled one-shot apply 内原子写入文件。production fault gate 未解除时
不会写文件。bootstrap 和 schema/migration 凭据只供 lifecycle job 使用，不得进入长期 API/worker runtime。
API map 不含 worker URL 或任何 ClickHouse 凭据；worker map 不含 API URL、reader 或 DDL 凭据。

要进入未来 one-shot apply，清单必须是 `schema_version: 4` 且 kind 必须是
`legacy_reference_migration`。旧 v3 仍可严格 `validate/inspect`，用于复查已有文件，但它没有
`execution` accessor，永远不能授权 apply；不能只改版本号把 v3 解释成 v4。v2 已退役并被明确拒绝。

v4 不再接受手填 `attestations`。writer 已停、队列已排空、restore 已通过和 source 已退役都是运行时才能
观测的事实，必须在动作成功后写入 hash-chain journal；预先写 `true` 不构成证据。v4 inspect 输出也不再
显示 v3 的 `operator_attestations_complete` 状态，以免把“不适用”误读成未完成或已完成。
同样，固定 reference commit、七个单值迁移策略以及“空节点”execution 不再手填；工具在严格解析
后注入它们，并把原始 JSON 与完整 hydrated 事实一起纳入 manifest HMAC。旧冗余字段会按
`deny_unknown_fields` 拒绝，不会形成新旧两种 v4 写法。

## 固定选择

`legacy_reference_migration` 不提供自由组合。下列前七项由 v4 自动注入，JSON 的
`decisions` 只填写仍然是真实人工选择的 `legacy_custom_rules`：

| 项目 | 固定值 | 结果 |
| --- | --- | --- |
| 旧配置 | `manual_only` | 目标值全部手填，不从旧配置导入 |
| 登录态 | `logout_all` | 不转换 Laravel session；用户和管理员切换后重新登录 |
| 旧 cache | `discard_ephemeral_after_fence` | 只在 producer 已 fence 后放弃已分类的临时 cache |
| Stripe | `assert_none` | 自动检查 Stripe 相关 inventory 为零，否则阻断 |
| 临时订阅链接 | `invalidate_at_cutover` | 不迁移 Redis 短期映射/cache；旧临时 URL 切换时失效 |
| 节点策略 | `one_shot_offline_cutover` | 不建设渐进切换；当前生产支持面进一步要求 inventory 为空，旧库存在节点时 inspect 明确阻断 |
| 旧 theme | `discard_confirmed` | 明确接受旧视觉/脚本资产不进入新版 |
| custom rules | `none` 或 `discard_confirmed` | 不盲拷旧模板；存在未处置文件时阻断 |

同时固定：

- `runtime.configuration_source=file_only`；
- 工具派生 API/worker config 固定路径、`C.UTF-8`、target 必须为空、principal 必须不存在、ClickHouse
  standalone 及最小权限声明；这些不是 v4 JSON 输入；
- native runtime 不含 query/form auth fallback、旧 JWT decoder/cutoff 或全局 node-token fallback；
- `runtime.server_require_idempotency_key=true`；
- `APP_KEY`、server token、lifecycle audit key 和所有 target 密码互不复用。

## 永久订阅 token 与旧 Redis

旧用户的永久订阅凭据是 source SQL 表中的 `v2_user.token`。它与用户 ID、`uuid`、密码 hash、余额、
套餐和流量一样属于必须原值转换到 PostgreSQL 的业务数据。

Redis 中的 `otp_`、`otpn_`、`totp_` 是短期订阅映射或验证 cache，不是永久 token：

- mode 1 通常先写双向 mapping，再签发临时 URL；
- mode 2 可由用户 ID、永久 token 和时间窗计算 URL，签发时完全不写 Redis；首次验证时才可能写 cache。

迁移不需要证明这些短期 key 为零，也不等待自然过期。它们随旧 cache 一起丢弃，旧临时 URL 在切换时
失效；用户在新版恢复后重新获取链接。mode 0 的永久 token 链接会因 `v2_user.token` 原值迁移继续有效。
为避免 mode 2 在相同时间桶复用算法导致旧临时 URL 继续有效，legacy v4 的目标
`runtime.show_subscribe_method` 一律只能手填 0 或 1；值 2 会被校验拒绝。清单不再要求操作者声明旧版 mode，
因为它不会改变“一律废弃旧临时 URL”的迁移结果。该选择不影响套餐、流量或永久订阅凭据。

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

这两个 prefix 和下文的 Horizon prefix 在 v4 都必须填写旧实例中的真实非空值；空字符串不能代表“没有
prefix”，否则 source drain 会扫描错误的物理 keyspace。只填一个 URL 可能产生危险的假零结果。当前
legacy v4 内部固定只支持 Redis cache driver，JSON 不再重复填写 `legacy_cache_driver`；真实来源若使用
其他 driver，必须先增加对应的只读 inventory adapter，不能伪填 Redis URL。

## v4 裸机 execution

`execution` 的所有结构都使用 `deny_unknown_fields`。操作者只填写真实外部输入和现场单元，不再复述固定
策略或内部路径。工具从 `operation_id` 生成 operation 专属目录，并以 owner-only 模式安全创建；执行时仍
逐级拒绝 symlink、错误 owner/mode 和 inode 替换。旧版清单若继续携带已删除的固定字段会被作为 unknown
field 拒绝，不会静默覆盖派生值。

- journal 根、activation state、authorization、receipt、备份输出/recipient/identity/state、retirement state
  和 lifecycle binary 路径都是固定或由 `operation_id` 派生的实现契约，不出现在 JSON。
- `release` 只填写安全的 content release ID 和 archive 的 64 位小写 SHA-256；archive 输入固定为 operation
  目录的 `inputs/native-release.tar.gz`，部署根固定为 `/opt/v2board/releases`，原子入口固定为
  `/opt/v2board/current`。该 archive 必须是 `root:root 0400` 的 regular file（不能是 symlink）；工具在同一
  已打开 inode 上完成 digest、结构化 tar 校验与安全解包，不接受可写的“待会再改”输入。
- `systemd` 只填写全部旧 writer/worker/scheduler
  unit 必须分类列出、全局唯一，且不能冒充新版 unit。每个 scheduler 必须同时声明同 stem 的 `.timer`
  与实际 `.service`，preflight 会核对 `Triggers/TriggeredBy`。授权后的第一步对 API、timer 和被触发的
  service 执行 `mask --now` 并复核 masked+inactive；queue worker 先 `disable` 防止重启复活，在现进程
  排空后再 `mask --now`。因此主机重启或 crash-resume 不会重新引入旧 writer。
- `source.database_url` 必须使用专用只读 principal。允许的 grant 只有 source database 上的
  `SELECT, SHOW VIEW`，以及全局只读的 `SHOW DATABASES, PROCESS, REPLICATION CLIENT, USAGE`。另需对
  `performance_schema.replication_connection_status` 和 `replication_group_members` 两张表作精确 `SELECT`
  授权；不允许扩大为整个 `performance_schema`。这些都不是写权限：它们让 inspect 能看见全部
  process/replication channel，避免权限不足时把主从或仍连接的 replica 错报为 standalone。任何
  INSERT/UPDATE/DELETE/DDL、`WITH GRANT OPTION` 或其他 database 的 SELECT 都会阻断。
- `source.database_fence_url` 必须指向与 `source.database_url` 完全相同的 MySQL `host:port`，但使用
  独立的 fence 用户和密码，且 URL **不得选择默认数据库**（不能带 `/v2board`）。该用户只允许全局
  `PROCESS, SYSTEM_VARIABLES_ADMIN`；工具用它执行并复核 `SET PERSIST super_read_only=ON`、等待活动
  InnoDB 事务归零，再由只读 principal 完成最终 fingerprint。给 fence URL 选择 source database 会因
  精确最小权限在握手阶段返回 MySQL 1044；不要为绕过 1044 给 fence 用户追加 `SELECT`。
- `source.redis_connection_prefix`、`source.redis_cache_prefix` 和 `source.redis_horizon_prefix` 必须填写
  Laravel 实例的真实非空物理 prefix，不能从 `APP_NAME` 猜，也不能用空字符串绕过扫描。
  `source_control.datastores` 对 MySQL、default Redis、cache Redis 只填写实际 `.service`。当前安全支持面
  刻意限定为本机 literal-loopback source；
  MySQL unit 必须独立，两个 Redis URL 指向同一 host+port 时必须填写同一个 unit，指向不同进程时必须
  填不同 unit。JSON 不再包含解析后必定拒绝的 `management`/`external_managed` 分支；远程或托管 source
  需要未来单独的 provider adapter 和新 schema，不能用授权前写好的 `control_applied=true` 文件代替动作。
- 不再存在 `provider_fence_receipts`。本机 API/scheduler 的 durable systemd fence 已覆盖 write ingress。
  远端 V2bX/XrayR 等节点是仓库外软件；本仓库没有其 node-side agent、远程进程控制或部署 inventory，
  因而不会伪造一个面板 API coordinator 来声称已经启动并验收远端进程。
- `execution.nodes` 不是 v4 JSON 字段。工具内部固定注入
  `activation_transport.kind=not_required_no_nodes` 和 `inventory=[]`；手填 `execution.nodes`
  会被当作 unknown field 拒绝，当前代码不存在远端节点激活协议。
- 在线 inspect 会在 repeatable-read 只读快照中精确读取旧 MySQL 的
  `shadowsocks/vmess/trojan/tuic/hysteria/vless/anytls/v2node` 八张节点表，生成 source node-set digest；
  任一表存在记录即返回稳定 blocker `external_node_coordinator_unavailable`，不会进入维护窗口。
- PostgreSQL bulk copy 后、native authority 提交前，production executor 会在同一只读快照中证明对应八张
  PostgreSQL 节点表与 `v2_server_credential` 都为零，并把稳定 empty-set proof hash 写入 journal/authority；
  authority 激活后、启动完成前再验证同一空集合和同一 proof。
- 要支持非空 inventory，必须把具体节点软件及 node-side 进程控制纳入新 schema 和端到端审计范围；不能在
  当前 empty-only schema 上追加一个面板 handler 或复活已经删除的 transport 字段。
- source retirement 使用固定的一次性 lifecycle binary 路径和 operation 派生的主动探针 state。已经实际解密、
  隔离恢复、精确指纹并清理 restore 的加密 backup 就是唯一永久 archive；其 runtime artifact SHA-256
  进入 backup receipt、journal 和 completion proof，不再制造第二份重复 cold archive。

`receipts.release_archive` 是授权前已经存在的不可变输入，JSON 只填写其精确 SHA-256，owner-private 路径
由 operation 派生。其余 receipt 路径不再是清单字段：source fence/drain、backup restore、单一
source retirement、legacy compatibility 禁用和 PostgreSQL authority 都只能在对应动作
真实完成后生成。每份动态 receipt 由 lifecycle audit key 做独立 domain HMAC，记录授权后实际执行时的
journal anchor generation/event/checkpoint；后继 journal event 再记录 receipt SHA-256，形成双向可恢复
hash chain。由 operation 派生的 source-retirement receipt 同时记录所有 dedicated unit 已
disabled+inactive，以及用清单内三组
旧凭据主动探测 MySQL/default Redis/cache Redis 均不可达。把未来 `completed=true` 或
`control_applied=true` receipt 的 SHA 预先塞进 manifest，会把承诺伪装成事实，v4 会因 unknown field 或
不支持的 external-managed source 直接拒绝。

### 可执行备份模式

v4 只接受 `mysql_logical_dump_and_isolated_restore`，不再让操作者选择 client family。执行器同时读取
`SELECT VERSION()` 和 `@@version_comment`，只接受 Oracle MySQL 8.0/8.4 source 与隔离恢复实例，并固定使用
root-owned `/usr/bin/mysqldump`（MySQL 8 client）与 `/usr/bin/mysql`。dump 使用
`--column-statistics=0`、`--no-tablespaces` 避免依赖服务端统计和宽泛 tablespace 权限。preflight 已禁止
source routines/events/triggers，所以 dump 是 table-only 且显式
`--skip-triggers`，不会为了不存在的对象索取额外权限。

source fence 后，执行器先做完整 canonical fingerprint，并把已经 HMAC/Journal 验证、包含所有冻结流量
deltas 的 source-drain receipt 与 dump 组成版本化二进制 frame，再把该 stream 直接送入 root-owned
`/usr/bin/age`。数据库密码只进入 operation 私有目录中的 `0600` defaults-extra-file，argv、environment、
receipt 和错误码均不包含密码；正常和失败路径都会删除该文件，进程崩溃后的重试只接受字节完全相同的
owner-only residue。recipient 和用于实际解密的 identity 是两个不同输入文件，路径和 SHA-256 都必须绑定。
restore 必须由 `age --decrypt --identity ...` 读取磁盘上的 `.age` 字节，先严格解析 magic/version、长度、
traffic receipt SHA/HMAC/anchor/delta 摘要，再只把 MySQL frame payload 流入 client stdin；没有明文 dump
或 traffic 文件，也不能用创建备份时的旁路明文冒充 restore drill。私钥禁止和 `.age` 一起放在持久
`/var/lib/v2board`：唯一允许路径是独立 owner-only runtime secret mount
`/run/v2board-lifecycle-secrets/<operation_id>/age-identity`，崩溃恢复时必须由外部 secret source 重新挂载同一
digest 的 identity，不能从 archive 目录复制。Completion 绑定 verified archive 后必须清除该 runtime mount；
永久归档只包含 `.age`、receipt 和 recipient，不包含解密 identity。

`command_timeout_seconds` 是 5 分钟到 7 天的 operator-bound 总 deadline；
`maximum_encrypted_backup_bytes` 是 16 MiB 到 16 TiB 的流式硬上限。写入前执行器要求输出文件系统至少还有
该上限加 1 MiB 的可用空间；示例的 24 小时、256 GiB 只是可支持十万至百万级数据集的起点，必须按线上
实际库和日志量填写，不能盲抄。partial 文件 fsync 后以 no-clobber hard-link 发布；若恰好在 link 与 unlink
之间崩溃，重试只会在两个名字确认为同一 root-owned inode 时完成收敛，其他冲突一律停止。

隔离 restore URL 必须同样指向 Oracle MySQL 8.0/8.4；其 credential 必须独立，并具备在
同一实例系统库上查询 `information_schema`、CREATE、DROP 指定 restore database 的权限。首次只接受
database 不存在或对象总数为零；执行器先写入 HMAC-bound
派生的 isolated-restore state 取得该 database 的 operation ownership，之后才允许在失败/重试中清理非空
restore。恢复后用与 source 完全相同的 canonical fingerprint 比较，随后从 system database 连接执行 DROP，
并再次查询 `information_schema.schemata` 证明目标不存在。state 只能按 reserved → dump committed → restore
in progress → destroyed 单调前进；SourceDrained checkpoint proof 允许 lost-ACK recovery 改变 journal
generation/event，但不能换掉原 proof。

第一次 restore drill 销毁后，executor 会从这同一个已验证 `.age` 在同一隔离 endpoint 上建立第二个、严格
operation-owned 的 archive materialization，后续 final seal、PostgreSQL copy 和 authority 前最终逐值复核
一律读取它，而不再把仍然在线的旧 MySQL 当复制来源。其独立 HMAC state 按
reserved → restore in progress → ready → destroying → destroyed 单调推进；ready 数据库每次使用都重算完整
fingerprint/schema。旧 MySQL 或旧 Redis 在归档完成后永久损坏时，只要全部旧 writer 的 durable systemd
fence 仍成立，迁移可依赖 archive 精确前向恢复；旧 MySQL 若仍可达则额外比对它未发生漂移。PostgreSQL
native authority 耐久提交后，必须先把 materialization 标为 destroying 并 DROP/复核不存在，之后才能启动
新版服务。它从不接受 runtime 流量，也不是 MySQL fallback。

由 operation 派生的 backup-restore receipt 是动作完成后生成的 HMAC 证明，绑定 backup reference、
加密 artifact SHA-256/字节数、
recipient/identity digest、实际解密、三份相同 fingerprint 和销毁证明。这个验证过的 `.age` 是迁移唯一的
永久 cold archive，不再另造第二份归档。内置 executor、全部 one-shot stage 和 CLI wiring 已经接线；
当前唯一剩余的生产开关条件是完整真实裸机 fault matrix 与最终安全审计，不能仅因本段通过就解除总
fail-closed gate。

`BackupRestoreVerified` 之后的数据转换不再依赖旧 MySQL 必须持续存活。执行器从上述 verified `.age`
重新建立 operation-owned isolated materialization，并用独立 HMAC state 固定
`reserved → restore_in_progress → ready → destroying → destroyed`。final seal、PostgreSQL copy、独立值复验及
authority 前最后一次数据复验都从这个只读恢复快照取数；旧 MySQL 仍可达时必须同时证明其 fingerprint/schema
未漂移，不可达时则要求所有旧 writer 的 systemd fence 仍然成立，并继续使用 archive 的精确数据证明。只有
`NativeAuthorityCommitted` journal event 已 fsync 后，执行器才先持久化 `destroying`、再 DROP 恢复库并写入
`destroyed`；DROP 或 state ACK 丢失都只允许继续销毁，不能倒退重建。这样旧 MySQL/Redis 在完整 archive receipt
落盘后永久消失也不会卡住单次迁移，但这不是 MySQL runtime fallback、CDC 或双写。

online `inspect` 会在唯一确认之前运行同一套只读 prerequisite probe：固定 binary 的 root ownership 与实际
client version、recipient/identity 文件 digest、source/restore Oracle MySQL 8 identity、source table+index byte estimate、
timeout/byte ceiling、输出目录可用空间、隔离 admin connectivity/version、source/restore identity 分离、目标库
absent-or-empty，以及从 `information_schema` 可观察到的直接 CREATE/DROP grant。缺少 age/private identity、
磁盘不足、restore endpoint 不通或权限只靠无法证明的间接角色都会直接成为 blocker，不能等 fence 之后才
发现。manifest 准备阶段因此必须预建 owner-only operation `inputs`、`outputs`、`receipts` 目录；probe 不会为
测试权限而创建或删除数据库。

## Target 由 lifecycle 创建

操作者提供已存在的 bootstrap principal 连接和外部网络控制证据，不手工预建目标业务库或长期
principal。production gate 解除后的 `apply` 必须在操作者根据在线 `inspect` 作出一次不可逆迁移授权后，
先写 durable pending journal，再 fence、排空并执行内部 final recheck，之后才可创建：

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

清单分别填写 bootstrap、schema、仅允许 raw/按批次日聚合 INSERT 与精确批次核对 SELECT 的 relay
writer，以及面向分析查询的 select-only reader principal。bootstrap
principal 已由基础设施提供；目标 database 及 schema/writer/reader principals 必须不存在。初始拓扑
固定为 standalone non-replicated，raw/aggregate retention 必须显式填写并满足 `aggregate >= raw`。

ClickHouse 只保存由 PostgreSQL outbox 投影、可重建的分析事实；短暂不可用时不阻止认证、订单、支付
的同步事务；流量结算在 manifest 冻结的 normal/soft PostgreSQL 容量窗口内继续。达到 hard rows/bytes/
oldest-age/headroom 或采样过期时，只有会新增 analytics event 的流量事务在提交前 503/重试，relay 继续
排空，认证、订单、支付不受该 gate 影响。当前 API 不消费 reader；worker 只得到 writer。API/worker
禁止同步双写 PostgreSQL 和 ClickHouse。

同一个 manifest 的 `target.analytics_admission` 必须手填 recovery/soft/hard pending rows、relation bytes、
oldest age、dedicated database capacity 与三档最小 headroom、event reservation、soft rows/s、采样与 stale
期限，以及可复查的 `capacity_evidence`。这不是旧配置导入项。lifecycle 在 ClickHouse stage 前把 policy
不可变地绑定 installation UUID，并把 policy hash、policy 全文及 DDL 前后两次精确 PostgreSQL
heap/index/TOAST/total/database/headroom 快照写入 stage report；初始状态不是 fresh+empty+normal 就停止。

legacy migration 的 ClickHouse stage 会用 schema principal 应用固定 26.3 lineage，把 manifest 中的
raw/aggregate retention 写成可逐表复核的 TTL，并与已保留的 installation UUID 绑定。两张 raw 表和两张
按批次日聚合表在切换前后必须全为空；`v2_stat`、`v2_stat_server`、`v2_stat_user` 已由 PostgreSQL
逐值验证报告覆盖，但不会被伪造成历史 ClickHouse 事件。relay 对 raw 与日聚合分别使用稳定 token 并
分别按 `ingest_batch_id` 全值复核；不依赖 dependent materialized view 的重试去重语义。

### Redis

目标 URL 必须使用 `rediss://`，与两个 source Redis identity 不同，并且选定逻辑 DB/namespace 为空。
Redis 没有“创建逻辑 DB”步骤；工具只验证空状态，绝不会用 `FLUSHDB` 帮忙变空。

## 先检查，再计划，最后才可能写入

固定顺序是：

1. `validate` 离线校验 v4 文件、全部必填 key/type、URL、secret 独立性、派生路径契约和固定决策。
2. 旧系统仍在服务时运行 `inspect`，在线只读检查来源、target 和当前 blocker。它不授权进入迁移。
3. 工具输出完整现场报告和稳定 `review_binding_sha256`。操作者查看后只作一次决定：是否针对精确
   `operation_id + review_binding_sha256` 启动不可逆的 one-shot `apply`。authorization 将这个稳定值保存为
   `inspect_review_sha256`，并把确认当时完整现场报告的 `report_sha256` 保存为
   `authorized_snapshot_report_sha256`；正常在线流量变化不会偷换结构/identity/policy，也不会迫使反复确认。
   拒绝或不确认都不进入维护、不停止旧服务，也不创建 target。
4. `apply` 先写入 fsync-durable pending journal，再在同一次调用中 durable fence 旧 API/scheduler，
   禁止 worker 重启、排空后停 worker，并对账流量、queue、failed/paid-pending work，建立一致 backup 并
   验证隔离恢复。可执行路径已由 preflight 证明旧库节点 inventory 为空。
5. 所有可能需要人工修复的 source admission 都在创建 no-clobber 唯一归档**之前**完成。归档与隔离恢复
   通过后只再做 durable fence、可达旧源的额外 no-drift、
   archive materialization 精确 fingerprint/schema 和 target empty 短复核，避免把可修复 blocker 留到不可变
   备份之后。
6. final recheck 通过后，同一 `apply` 才创建 target、bulk 转换、逐值验证、物化
   配置并只启动新版 API/worker 一次；验收成功后立即完成 source retirement。

`apply` 不得实现 CDC、双写、shadow read、跨维护窗口 backfill 或按节点逐批恢复服务。journal/checkpoint
只用于同一次停机操作的崩溃恢复。最终提交后统一启动新版，随即永久 mask 本机 source runtime、
MySQL 8 与旧 Redis dedicated systemd units，使用旧凭据主动证明三个 endpoint 均不可达。不可变
PostgreSQL completion ledger 成功后，结果会输出 root 人工清理 argv；操作者自行删除一次性
`v2board-lifecycle`，工具是否已删不属于迁移成功证明。实际解密恢复和精确指纹通过的加密 backup 是唯一
带 SHA-256 的永久 archive。
此后禁止 MySQL runtime rollback，只能恢复 PostgreSQL/ClickHouse 或 forward recovery。

当前 CLI 同时解析 `apply` 和 `resume`，但单一 typed production capability 仍固定为不可用；validate、
inspect verdict/next action、authorize readiness、apply 和 resume 全部从该能力值派生，调用会在任何
写入前失败。`authorize` 会重新执行在线只读检查，要求稳定 review binding 与操作者复查值一致，并把
当前完整 report 作为授权审计快照；确认前后都不会 fence 或修改 datastore。typed converter、durable
journal、完整 executor 与裸机 activation 已接线；16 个有外部副作用的阶段已完成 before-effect、lost-ACK、
进程终止三种边界，共 48 个确定性恢复用例，但尚未完成解除总开关所要求的真实裸机 crash matrix。报告会显示
`converter_available=true`、`apply_available=false`、`verdict=blocked`、
`next_action=resolve_blockers`。即使某些现场检查全部通过，也不能手工把
它解释成受支持迁移许可。

## 使用 v4 示例

模板含公开 placeholder，不能直接运行。复制到仓库外，逐项复查并填写，生成新的 operation UUID，
限制为 owner-only 权限：

```bash
cp docs/examples/legacy-migration.v4.example.json /secure/private/legacy-migration.json
chmod 600 /secure/private/legacy-migration.json

v2board-lifecycle validate --manifest /secure/private/legacy-migration.json
v2board-lifecycle inspect --manifest /secure/private/legacy-migration.json
```

清单必须是 regular、non-symlink 文件，大小为 1 byte 到 1 MiB，且在 Unix 上不得授予 group/world 权限。
datastore URL 密码中的特殊字符要 percent-encode。生产 target 必须使用经过身份验证的 TLS；source 只有
在具名可信维护网络或加密隧道内才可声明非 TLS 例外。本机 loopback 维护连接若明确使用该例外，应显式写
`?ssl-mode=DISABLED`；不能依赖 client 的 opportunistic TLS 自动降级。

报告不会输出明文 secret。`manifest_binding_hmac_sha256` 绑定原始清单 bytes；
`review_binding_sha256` 及其 HMAC 只绑定需要人工复查且应保持稳定的 source/target identity、schema 和 policy；
`report_sha256` 及 `report_binding_hmac_sha256` 绑定包含动态计数在内的完整当次检查 payload；规范化时仅把这两个
输出摘要槽保留为空，随后把计算结果写入最终 JSON，因此它不是对“已经包含自身摘要的打印 bytes”再次裸 hash。
authorization/journal
使用 `inspect_review_sha256` 表示前者，并另存 `authorized_snapshot_report_sha256` 表示确认时的后者，二者
不得混称为同一个 inspect report digest；
`legacy_execution_binding_hmac_sha256` 可供裸机 policy 构造器只消费 execution 子树，同时仍受整份 raw
manifest binding 约束。修改 JSON 空格、凭据、endpoint、operation ID、release/node input digest、
backup reference 或 runtime 值都会改变整份 binding。

## 当前只读检查覆盖与缺口

legacy `inspect` 已检查或报告：

- source Oracle MySQL 8.0/8.4 的实际 vendor/version、read-only session、结构指纹、
  standalone 可见拓扑和 core object inventory；
- 未完成/paid-pending order、Stripe 零状态、关系/唯一性/collation 冲突、giftcard redemption；
- 旧 MySQL 八张节点表的精确空集合与 node-set digest；非空时返回稳定 blocker；
- 节点 `group_id`、coupon `limit_plan_ids`、order `surplus_order_ids`、giftcard `used_user_ids` 的 JSON ID
  array 类型、正整数/目标范围、引用完整性和数字字符串 normalization 计数；
- 两个 source Redis 的流量、queue、failed work、reset lock、临时订阅 cache 数量与实例 identity；临时
  订阅 cache 只报告、不阻断；
- target PostgreSQL 18 能力、库/角色为空、collation 与外部控制声明；
- target ClickHouse 26.3 LTS、database/principal 为空、standalone 状态、grant/retention 声明；
- target Redis 为空、版本/命令能力与 source 隔离。

进程内 48 场景矩阵已经覆盖每个有副作用阶段的 effect 前失败、effect 后 ACK 丢失和执行器进程终止重建，
包括 archive 建立后同时丢失旧 MySQL/Redis 的恢复。持续 fail closed 的核心缺口是完整真实环境 fault matrix
与最终审计：真实 systemd 进程/宿主机重启、fsync 后掉电、网络分区、磁盘 ENOSPC/inode 耗尽、挂载丢失、
age 解密/恢复中断和组合故障都必须证明只能精确前向恢复。当前安全支持面只含本机 dedicated systemd
source；清单没有 external-managed 分支，不能用人工 provider receipt 扩大范围。
ClickHouse 固定单节点的 schema lock、崩溃恢复、精确 lineage、installation binding 以及
installation-bound raw/按批次日聚合 TTL 已实现；它是可从 PostgreSQL/outbox 重建的投影，HA/Keeper 是
部署可用性选择而非一次迁移正确性的伪 blocker。PostgreSQL outbox 的 manifest-bound 容量预算、精确 relation/database 采样、
normal/soft/hard 背压、pre-commit rollback、relay drain 和 hysteresis recovery 已实现；它不替代宿主机
磁盘/WAL 告警。secret split 或任一阶段单独通过都不会解除总 fault gate。

Docker `make rust-integration` 还固定运行 Oracle MySQL 8.4 完整 inspection/query surface、边界 JSON、真实
`age + mysqldump + mysql` 加密备份/隔离恢复/二次重物化、Redis Lua 原子流量冻结与精确重试、PostgreSQL
traffic fold/ACL，以及 PostgreSQL → ClickHouse outbox replay。这些是真实 datastore 集成门，但不能冒充
Linux systemd、宿主机掉电、ENOSPC、inode/挂载丢失和网络分区的裸机证据。

普通 `v2board-api migrate` 只运行已确认 native PostgreSQL lineage 的 SQLx migration；它不是 legacy
adoption/converter 命令，绝不能指向旧 MySQL 或未知 PostgreSQL 数据库。
