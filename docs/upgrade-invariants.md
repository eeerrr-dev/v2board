# 安装、旧版迁移与升级不可变契约

状态：**F0 架构与唯一 legacy schema v5 archive-first 策略已建模；production apply 继续 fail closed**

本文定义三条且仅三条 lifecycle 路径：

- [`fresh_install`](examples/fresh-install.v3.example.json)：全新安装；
- [`legacy_reference_migration`](examples/legacy-migration.v5.example.json)：从唯一旧版迁移；
- [`native_upgrade`](examples/native-upgrade.v3.example.json)：已安装新版的升级，包括未来破坏性更新。

新版唯一权威事务库是 PostgreSQL 18，ClickHouse 26.3 LTS 只保存可重建分析投影，Redis 只保存 session、
lease、锁、限流和短期 cache。完整数据库所有权、outbox、版本与故障语义由
[PostgreSQL + ClickHouse 持久化不变量](postgresql-clickhouse-invariants.md)定义；本文件不重复放宽其中
任何要求。

唯一旧版来源是 `references/wyx2685-v2board` commit
`7e77de9f4873b317157490529f7be7d6f8a62421`。唯一允许的旧数据库来源是 Oracle MySQL 8.0/8.4；
MySQL 5.7、Percona、MariaDB 和兼容代理全部拒绝。旧站停机后生成的 age-encrypted MySQL dump 是
`legacy_reference_migration` 的唯一数据输入；迁移器不连接 live 旧 MySQL。MySQL 绝不是新版 runtime
target，也不存在 MySQL 与 PostgreSQL 可选双后端。

本文是目标规范，不代表生产 apply 已经启用。当前 CLI grammar 包含：

```text
v2board-lifecycle validate --manifest <path>
v2board-lifecycle inspect --manifest <path>
v2board-lifecycle apply --manifest <path>
```

fresh/native v3 目前只是未来目标规范；当前 lifecycle loader、`validate` 和 `inspect` 都拒绝它们。
当前唯一接受的是严格的 legacy cold import `schema_version: 5`，不存在兼容分支。
v5 明确选择放弃旧 Redis 全部状态、Stripe 本地活跃状态、旧节点、历史流量明细以及 `v2_log`、
`v2_mail_log` 瞬态历史。所有退役 shape 都被严格拒绝。
legacy v5 没有 `authorize` 或 `resume`。当前 typed production capability 控制 apply；它尚未启用，所以
`validate/inspect` 报告 `apply_available=false` 与 incomplete importer/operation-owned-cleanup blocker，且
`apply` 会在任何 target 写入前被拒绝。`v2board-api migrate` 只是已确认 native
PostgreSQL lineage 的 schema runner，不是全新安装 orchestrator，也不是 legacy converter。

## 1. 保护等级与全局规则

| 等级 | 定义 | 允许变化 |
| --- | --- | --- |
| F0 — 保值与追加历史 | 身份、权威数据、审计历史、业务语义和永久外部契约 | 不得静默修改、删除、重新解释或回退；只可追加历史、单调推进 epoch，或执行经证明语义等价的表示迁移 |
| F1 — 协调轮换 | secret、域名、路径、principal、物理 endpoint 等可轮换状态 | 必须显式、可审计，具备消费者清单、验证和回滚 |
| F2 — 有界兼容 | 旧协议、旧凭据、旧资源或回滚 artifact | 必须有 drain/采用率证据；无法证明匿名消费者为零时，使用所有者批准的最大窗口、通知、截止点和失败策略 |
| F3 — 可重建或明确放弃 | 非权威 cache/build output，或已 inventory 且明确接受损失的 artifact | 删除后可由 F0/F1 重建，或有具名接受记录且回滚窗口已经结束 |

以下规则永久生效：

1. 未分类的表、列、配置、文件、Redis key、事件、外部行为或第三方 payload 一律按 F0。
2. 有歧义就停止；不得猜值、取第一条、自动 dedupe、自动取消、自动补默认或自动挂到默认父记录。
3. SQL 成功、进程启动或 `/readyz` 成功均不能单独证明 lifecycle 完成。
4. 没有消费者退役证明就不能删除兼容层；代码搜索和“应该没人用了”不构成证明。
5. 普通版本升级不得夹带 secret、用户 token、节点凭据、域名、路径或 datastore 轮换。
6. 新默认可以用于 fresh install；既有安装必须物化旧有效值或经过显式配置迁移。
7. 存储表示可以迁移，值、单位、状态、关联、舍入、时区和外部可观察结果不得漂移。
8. installation UUID、完成的 operation/migration ledger 和历史 release binding 是 append-only F0；
   schema/config/data/credential/session/traffic epoch 只能单调推进。
9. 任何写路径均须先有可恢复 journal、backup/restore proof 和明确确认；未实现的 proof 必须成为 blocker，
   不能降级成 warning 后放行。

“不变”约束 lifecycle，不禁止正常授权业务动作。例如用户主动改密、`resetSecurity` 或管理员封禁可以
按既有业务契约改变状态；升级器不得冒充这些动作。

## 2. 来源分类与 lineage

第一笔 mutation 之前必须只读分类：

| 分类 | 条件 | 唯一允许路径 |
| --- | --- | --- |
| `empty` | target PostgreSQL/ClickHouse/Redis 无业务 schema、principal、ledger 或残留安装状态 | `fresh_install` |
| `legacy-reference-supported` | encrypted dump 摘要有效，隔离恢复后的 schema 命中 pinned profile，target 无 native ledger，所有损失决策完整 | `legacy_reference_migration` |
| `legacy-reference-drift` | 可识别为 reference，但结构、数据、配置或 artifact 有未解释差异 | 只读差异报告；禁止写入 |
| `native` | installation identity、PostgreSQL/ClickHouse ledger、runtime、Redis 和 schema/config epoch 一致 | `native_upgrade` |
| `recoverable-pending` | native pending operation 的 installation、target identity、plan checksum、checkpoint 和 backup 可验证 | 仅该 native schema 明确支持的 recovery；legacy cold import 清理后从 dump 重跑 |
| `dirty-or-unknown` | 半安装、混合 lineage、checksum 异常或其他未知状态 | 禁止写入 |

分类不得只看表名。legacy profile 至少覆盖列顺序、类型、nullable/default、生成列、主键、唯一键、索引、
外键/check、charset/collation 及已知残留；source vendor/version、SQL mode、拓扑和 capability 单独探测。
旧更新器可能忽略单条 SQL 错误，因此同一代码 commit 也必须检查真实 schema drift。

reference 只能用于只读 contract 审计、fingerprint 和 fixture。生产 release artifact 不得执行、复制或回退到
reference。若 reference commit 改变，必须新增具名 source profile/adapter，不能静默改变现有 adapter。

PostgreSQL 使用独立的 final-state baseline 与后续 append-only SQLx migration lineage；历史 MySQL
migration 的版本/checksum 不得伪装成 PostgreSQL 已执行。首个 native 正式发布后，任何已被不可丢弃
database 记录成功的 migration 都不得修改、重编号、删除或 squash。

ClickHouse 使用独立 schema ledger；不得借用 PostgreSQL `_sqlx_migrations`。

## 3. 三条 lifecycle 路径

### 3.1 Fresh install

只接受严格空 target：

- PostgreSQL 18 target database 与 migration/API/worker roles 必须不存在；已存在的 bootstrap role 只供
  lifecycle 创建这些 target 对象；
- ClickHouse 26.3 target database 与 schema/writer/reader principals 必须不存在；已存在的 bootstrap
  principal 只供 lifecycle 创建这些 target 对象；
- Redis 选定 logical DB/namespace 必须为空；绝不以 `FLUSHDB` 制造空状态；
- PostgreSQL collation/ctype 固定 `C.UTF-8`，ClickHouse retention 显式填写；
- `pg_hba.conf`、network policy、ClickHouse grant 和容量证据由外部控制并写入 spec。

future apply 必须先在耐久外部/runtime journal 写入 pending installation，再创建 database/roles/schema，
只生成一次 installation UUID。失败重试必须恢复同一 operation，不得重新生成 identity/secret 或接管
其他 operation 留下的对象。不得使用宽泛 `IF NOT EXISTS` 把未知对象变成成功。

生产不创建测试套餐、知识内容或固定管理员密码。首个管理员 secret 只可通过 stdin、受限 secret file/
manager 或一次性 bootstrap token 交付，不进入 argv、日志或 shell history。PostgreSQL、ClickHouse、
Redis、runtime config、frontend 与最小权限验证全部成功后，才能原子标记 installation active。

### 3.2 Legacy reference migration

只接受 `legacy-reference-supported`。唯一 v5 固定为 archive-first destructive cold import。操作者完全
停止旧站后生成 age-encrypted MySQL 8 dump；manifest 绑定 encrypted bytes、identity、隔离恢复数据库、
全新 target、runtime、release 和每项损失。具体字段见[旧版迁移清单 v5](legacy-migration-manifest.md)。

固定顺序：

1. 操作者在迁移器外停止旧 API、worker、scheduler、支付入口和远端节点报告者。
2. 操作者生成完整 MySQL 8 dump，age-encrypt，记录 SHA-256，并清理临时明文。
3. `validate` 严格校验 manifest；当前 `inspect` 只安全打开并 hash encrypted dump、age identity 和 release，
   对隔离恢复与 target 只检查静态声明，不连接旧站或 live target。
4. production gate 解除后的单次 `apply` 从同一 dump 建立 operation-owned 隔离恢复，验证 pinned schema，
   然后创建空 PostgreSQL/ClickHouse/Redis target。
5. converter 复制保留数据、验证关系/值/target 空表，生成 API/worker role-owned config，并在全部
   pre-activation 检查通过后一次启用 native authority。
6. authority 启用前失败时，删除隔离恢复库和全部未激活 target，以新 operation ID 从同一个 dump 重跑。

legacy v5 没有 authorization file、durable stage resume、旧 MySQL live URL、旧 Redis URL/prefix、旧
systemd unit 或 datastore fence。它不建设 CDC、双写或长期 bridge。

`legacy_redis=discard_all_without_inspection` 明确牺牲 pending traffic、queue/failed work、session、OTP、
临时订阅链接、cache/lock/lease/rate-limit 和 Horizon metadata。永久 `v2_user.token` 与 MySQL 已落盘
`u/d` 保留。`failed_jobs` 直接丢弃。

Stripe 配置与 status 0/1 Stripe 订单不复制，provider objects 固定 `ignore_uninspected`；工具不访问 Stripe。
status 2/3/4 Stripe 订单保留业务历史，但 `payment_id=NULL`、`callback_no=NULL`。非 Stripe payment 配置和
未完成订单作为普通业务数据保留。`v2_user.balance` 原值迁移，不因丢弃 Stripe 状态自动退款、补偿或重算。

八张节点表、route、credential、`v2_stat_user`、`v2_stat_server`、`v2_log` 和 `v2_mail_log` 不进入 target；
`v2_stat` 继续逐值保留。旧数据整体由 encrypted dump SHA-256 绑定，无需 live-source 27 表 fingerprint、
Redis receipt 或每张放弃表的单独 source proof。迁移后由操作者使用新 token 手工重建节点。

旧 `.env`、PHP/Laravel 配置、theme 和 operator script 不扫描、不导入；manifest 生成新的 role-owned config。
拒绝或 fail-closed 的 `apply` 不创建 target。pinned reference、旧库和 encrypted dump 都永不作为生产
runtime 回滚；dump 只是永久冷归档与可重复导入输入。

### 3.3 Native upgrade

只接受安装 identity 和 ledger 可机器验证的 `native`：

- 当前/目标 build ID、PostgreSQL epoch、ClickHouse epoch 显式且单调；
- migration/API/worker PostgreSQL principals 指向同一 installation，ClickHouse schema/writer/reader
  principals 与 runtime config 绑定；
- 当前 v3 strategy 固定 `maintenance_cutover`；未实现 N/N-1 compatibility matrix 前不宣称 rolling；
- PostgreSQL 18 patch、未来 PostgreSQL major、ClickHouse 26.3 patch/未来 next-LTS 分成独立计划，不能在
  同一窗口同时跨两个 database major/LTS。

任何 destructive change、TTL shortening、drop 或 repartition 都必须在 spec 中逐项写出 resource、
impact 和 rollback。存在破坏项时还必须显式允许、完成 impact review、绑定 backup/restore proof，并用
同一 operation ID 和 prior v3 report SHA 做第二次确认。

## 4. Installation identity、journal 与发布绑定

database、runtime directory、config、Redis namespace、secret version 和 release 必须证明属于同一个非零
installation UUID；相同库名、systemd unit 名或 endpoint 不足以证明。

F0 ledger 至少记录：operation ID/kind、installation ID、source/target fingerprints、build/schema/config/
data epochs、plan/report checksum、backup reference、step/checkpoint、操作者、时间和状态转换。

第一笔受保护 mutation 之前，权威 journal 必须已经以 fsync 耐久写入 `pending`。随后只以 append-only
event 与 CAS head 单调推进 `running -> verifying -> completed`；失败追加 `failed` 或
`needs-recovery`，保留最后 checkpoint。journal 不存明文 secret。

每个 release 有不可变 build manifest，至少声明 source/image/migration/checksum、支持的 PostgreSQL/
ClickHouse/config/data/job epoch、reader/writer 范围、upgrade capability、frontend content ID、asset
retention 和回滚范围。每个 installation 另有历史不可变 deployment binding，绑定 release digest、
datastore/runtime identity、operation phase、secret version/HMAC fingerprint 和 active frontend release。

readiness 必须联合验证 ledger、installation identity、schema checksum 与 release compatibility。too-old、
too-new、drift、wrong installation 和 API/worker mismatch 都要 fail closed。

## 5. 权威数据与转换不变量

### 5.1 通用保值

- 所有主键、自然键和父子关系原值保留；PostgreSQL sequence 必须高于现存最大值且不复用历史 ID。
- soft-deleted、archived、disabled、cancelled 和 completed 行仍是历史，不因 UI 不显示而删除。
- `NULL`、0、空字符串、空 array 和字段缺失不得折叠；未知 JSON/text 字段按不透明内容保留。
- created/updated/paid/first-seen/last-seen/stat 时间不得改成迁移时间。
- legacy collation 到 `C.UTF-8` 前必须检测大小写、重音、宽窄、尾空格、Unicode normalization 和截断
  collision；不得自动挑一个重复账号或 code。
- PostgreSQL runtime 默认 `READ COMMITTED`；依赖更强隔离的事务对 `40001/40P01` 做整个事务的有界
  jitter retry。database 时区 UTC，业务日界线固定 Asia/Shanghai。
- native lifecycle 的每个步骤必须可检测、可重放或可恢复；不能留下“DDL 已提交但 journal 未记录”的
  无主状态。legacy v5 是明确例外：authority 启用前失败时清理本 operation 的隔离恢复与未激活 target，
  再以新 operation ID 从同一 encrypted dump 完整重跑，不做 durable stage resume。

### 5.2 用户、认证和 credential

用户 ID/email/password/hash algorithm/salt、token/uuid、invite/Telegram 关系、role/ban、plan/group、余额、
佣金、u/d/quota/expire、设备/速度限制、提醒、session/traffic epoch、登录信息和时间均为 F0。

旧密码摘要逐 byte 保留；只有成功验证明文后才可 CAS 惰性升级 Argon2。迁移不得强制 reset password。
email 在写入边界规范化并唯一，但 legacy collision 必须先报告，不能自动 lowercase 后 dedupe。
session/traffic/credential epoch 只能保持或增加；升级不得让被撤销状态复活。token+uuid 只由授权
`resetSecurity`/admin reset-secret 动作轮换。

legacy 新字段固定初始化：现存 user 的 `session_epoch=0`、`traffic_epoch=0`；不为已放弃的旧节点创建
credential 或 credential epoch；保留 payment 的 `archived_at=NULL`；保留 order 的
`callback_no_hash=NULL`。其他新字段必须有具名 mapping/provenance，不能交给会变化的默认值。

### 5.3 订单、金额、佣金和支付

除 3.2 节具名的 Stripe 破坏性例外外，order ID/trade number/callback、user/invite/plan/coupon/payment
binding、type/period/status、金额、paid time、surplus relation 和 commission history 全部原值保留。
固定枚举包括：

- order type `1/2/3/4/9`：新购/续费/换套餐/重置包/充值；
- order status `0/1/2/3/4`：待支付/开通中/取消/完成/折抵；
- commission status `0/1/2/3`：待确认/发放中/有效/无效；
- `plan_id=0 && period=deposit` 是充值，不是 orphan。

金额、余额、手续费、优惠、退款、佣金和 giftcard 金额以整数 cents 存储；管理端元/分换算固定 100。
流量以 bytes，套餐管理 GiB 使用 `1_073_741_824 bytes`。coupon type 1 是 cents、type 2 是百分数点。
server rate 使用十进制；legacy 非数字/空 rate 按 0。durable quota 分别计算
`round(raw_u*rate)+round(raw_d*rate)`，每项 midpoint-away-from-zero；展示 charge 的
`(raw_u+raw_d)*rate` 是独立契约，不能合并或改用未经证明的二进制浮点。

native 与 legacy 中保留的非 Stripe payment `(provider,uuid)`、历史 config、验签材料、callback identity
和 reconciliation 永久保留。支付配置是不可变验签版本：轮换要 archive 旧 row、创建新 row；旧 row不用于
新 checkout，但继续验证迟到 callback。签名/metadata/金额不足只能进入 reconciliation，不能降低验签要求
自动开通。legacy v5 唯一例外是丢弃 Stripe 配置及 status 0/1 Stripe 订单、不访问 provider，并为保留的
status 2/3/4 Stripe 历史清空 `payment_id` 与 `callback_no`；这不会改变 `v2_user.balance`。

### 5.4 套餐、code、节点、工单和内容

plan/group/coupon/giftcard/invite code 的 ID、code、限制、价格、有效期、使用/redemption 和关系均为 F0。
coupon/giftcard/invite code、user email/token、order trade number、每用户一个 retained unfinished order、
每用户一个 open ticket、保留 payment `(provider,uuid)`、redemption、reconciliation、traffic/mail
idempotency 等唯一结果不得自动修复。

`giftcard.used_user_ids` 转换按不同 user ID 的集合语义；malformed/nonexistent user 阻断。旧格式无真实
redemption time，固定写成 `created_at=0, created_at_provenance=legacy_unknown`；新版兑换固定使用
`created_at_provenance=native` 且时间必须大于 0，不能把迁移时间伪造成历史兑换时间。

legacy source JSON ID array 的保留数据门禁覆盖以下三类列；节点 `group_id` 随节点整表丢弃，不读取
成员或将其 violation 变成 blocker：

- `v2_coupon.limit_plan_ids`，引用 `v2_plan.id`；
- `v2_order.surplus_order_ids`，引用 `v2_order.id`；
- `v2_giftcard.used_user_ids`，引用 `v2_user.id`。

非 NULL 值必须是 JSON array。coupon/order/giftcard 的 SQL NULL 是旧 schema 定义的“未设置”，允许保留；
顶层 JSON literal `null`、scalar 和 object 都是非数组 violation。成员只允许正整数 JSON number，或严格匹配
`[1-9][0-9]*` 且不超过目标 ID 类型上限的十进制字符串。合法字符串按成员数计入
`requires_normalization`，不作为数据 violation；one-shot converter 必须把它们写成 JSON number。零、负数、
浮点/指数 number、带符号/空白/前导零等歧义字符串、整数溢出、嵌套值及不存在的引用均计入
`violations` 并阻断。报告按三类列分别给出 scanned/null/array、normalization、format、missing-reference 和
total violation 计数；normalization 计数绝不等于 apply 已可用。

schema v5 不扫描已明确放弃的节点 `group_id` 行；coupon、order、giftcard 等保留数据的任何 violation 仍
阻断。discard 表留在 encrypted dump 中，不能把未经检查的 source 伪报为合法。

节点身份是 `(node_type,node_id)`；普通 native 升级必须保留协议字段、route、rate、port、parent 和
credential epoch，且不得改变 ID/type。schema v5 legacy migration 是唯一具名例外：不复制旧节点、route
和 credential，清空 target sequence 后重新建立，因此不允许旧 token 或 node identity 继续生效。

ticket/message ID、owner/author/status/reply status/time，knowledge/notice language/category/body/tag/sort
均保留。普通 native upgrade 按 retention 保留 mail/application logs；legacy v5 明确放弃旧
`v2_log`/`v2_mail_log` target rows，完整旧值只留在 SHA-256 绑定的 encrypted dump 中。native durable mail
outbox 始终保留。`ticket_message.user_id=0` 是合法 admin author，不可当 orphan。

### 5.5 统计与 ClickHouse

PostgreSQL 是 user quota、订单、支付、idempotency 和 outbox 的唯一 authority。API/worker 禁止同步双写
ClickHouse。`traffic.reported.v1` 和 `traffic.accounted.v1` 在同一 PostgreSQL 业务事务写 outbox，relay
以 immutable batch、稳定 dedup token 和完整内容核对发布；短暂 ClickHouse failure 不同步阻塞核心
事务。长期故障受 installation-bound outbox 容量、relation/database bytes、最老 pending age、磁盘
headroom 和 sample freshness 的 normal/soft/hard 门禁约束；soft 限速，hard/stale 只回滚新增分析事件的
流量事务，relay 始终继续排空。

legacy v5 逐值保留 `v2_stat`，放弃 `v2_stat_user` 与 `v2_stat_server`。encrypted dump SHA-256 绑定整个
旧库；迁移结果证明 target 不含两张明细表的旧行，但不再为每张放弃表建立 live-source digest。
`v2_user.u/d/transfer_enable` 按 MySQL dump 原值保留；未落 MySQL 的 Redis 尾流量明确丢失。
ClickHouse 必须从空的 native event epoch 开始。旧聚合不展开、不估算、不伪造成不存在的 raw event。
迁移后 relay 对 raw 与按批次日聚合分别使用稳定 dedup token 和完整批次核对；不能依赖 dependent
materialized view 在 ambiguous retry 下替代幂等证明。ClickHouse 任意投影都不得反写或决定订单、支付、
quota、subscription eligibility 与迁移成功。

## 6. Legacy session、Redis、订阅和节点切换

唯一 legacy migration 固定 `logout_all`：

- 不转换 Laravel/JWT/native session；不读取 source Redis，新 target Redis 为空；
- native runtime 不包含 query/form `auth_data` fallback、旧 JWT decoder 或 legacy cutoff 配置；
- 切流后所有旧 user/admin token 必须 403，客户端清 `localStorage["authorization"]` 并回登录；
- password hash 仍原值迁移，重新登录时才按正常规则升级。

旧 SQL `v2_user.token` 是永久订阅凭据，必须原值转换。旧 Redis 中的 OTP/TOTP、mode 1/2 临时订阅 URL
映射与 cache 全部随 Redis 一并丢弃；不等待过期，也不作为 blocker，用户在新版重新获取。
`runtime.show_subscribe_method` 只描述新版目标行为，不用于恢复旧临时链接。

`legacy_redis=discard_all_without_inspection` 是完整且不可拆分的接受损失：迁移器没有旧 Redis URL，不做
SCAN、freeze、drain、fold、对账或 key classification。尚未落 MySQL 的 pending traffic、queue/retryable
failed work、session、OTP、临时 URL、cache、lock、lease、rate-limit、幂等临时状态和 Horizon metadata
全部丢弃；MySQL `failed_jobs` 也丢弃。新 Redis 必须为空，不能以 `FLUSHDB` 制造空状态。

节点固定为 `discard_and_manual_rebuild`：旧节点、route、credential 不迁移，target 从零节点开始；迁移器
不审计或激活仓库外远端进程。操作者在导出前停止旧报告者，迁移后使用新 `server_token` 手工重建。
旧 Redis 中的 reset lock/status 同样不读取；native scheduler 从 manifest 与已迁移 MySQL 值建立新状态。

## 7. 外部 API、URL、订阅和第三方契约

真实客户端、节点或第三方消费的 route/method/parameter location/name/case/empty semantics、form array、
header、content type、status、envelope、ID、单位、NULL 和幂等结果均为 F0。可以追加兼容字段；删除、
改名、改类型、改单位必须新版本或双读写 + F2 退役。

F0 包括 `/api/v1`、已有 `/api/v2`、动态 admin/staff path 内的 API、原始 authorization、
`Content-Language`、`Idempotency-Key`、`x-v2board-step-up`，以及 endpoint-specific envelope/ACK。403
清 auth/跳登录与站内 redirect 安全结果不能漂移。

已发布邮件、书签、客户端和第三方 URL 必须继续解析，包括 login verify/redirect、register code、
forgetpassword、subscribe、order trade number、ticket ID、knowledge ID、payment/Telegram callback 和
订阅 path。当前 user hash routes 为 `/`、`/login`、`/register`、`/forgetpassword`、`/dashboard`、
`/plan`、`/plan/:plan_id`、`/order`、`/order/:trade_no`、`/profile`、`/invite`、`/ticket`、
`/ticket/:ticket_id`、`/knowledge`、`/node`、`/traffic`。动态 admin outer path 是 F1；其内部 routes 与
auth/redirect 结果是 F0。

永久订阅 token/uuid、mode 0 URL、mode 0/1/2 verifier、UA/flag、headers、General/Base64、Clash、Stash、
sing-box、Surge、Surfboard、Loon、Quantumult X、V2RayTun、SIP008、Shadowrocket、SagerNet 等 native
输出语义为 F0。改变未来 subscribe host/path/mode 属于 F1；legacy v5 的唯一例外是旧 Redis 完全不读取，
因此旧 mode 1/2 临时 URL 与映射在切换时无条件失效，永久 token 和旧 subscribe path 仍必须保持。

支付 callback path、provider code/UUID/config/signature/raw body/metadata、checkout request、return URL、
response shape 和 exact ACK 对 native 与保留的非 Stripe provider 均为 F0。现有 provider 包括
`AlipayF2F`、`BEasyPaymentUSDT`、`BTCPay`、`CoinPayments`、`Coinbase`、`EPay`、`MGate`、
`StripeALL`、`StripeAlipay`、`StripeCheckout`、`StripeCredit`、`StripeWepay`、`WechatPayNative`。
legacy v5 直接丢弃大小写不敏感地以 `stripe` 开头的 payment 配置及其 status 0/1 订单；status 2/3/4
订单只保留业务历史，并把 `payment_id`、`callback_no` 置为 NULL。工具不调用 Stripe API，不枚举、不取消、
不结算 provider object，因此 provider-side object 不是 blocker；旧库中 status 不在 0..=4、关系损坏或格式
非法的 Stripe 本地行仍 fail closed。非 Stripe payment 配置和未完成订单仍按普通保值规则转换。

Telegram binding/commands/join approval/webhook idempotency、邮件 code/URL/pending outbox identity、Crisp/
Tawk user/session payload 的身份、shape、单位与用户隔离均为 F0。bot token、SMTP transport、domain 和
webhook secret 是 F1；改变时必须重注册/验证，不能泄漏或重放上一用户数据。

前端像素、spinner/toast/modal/poll timing 和纯显示格式不是 lifecycle F0；但映射到 payload、URL、
数据、security 或 external integration 的结果仍为 F0，并必须由 behavior/interaction contract 覆盖。

## 8. Runtime config、secret 与 mutable files

legacy 固定 `manual_only`：旧 `.env`、PHP config、theme/operator script 不执行、不 merge、不导入，也不由
lifecycle 工具遍历任意旧文件系统路径来生成存在性/type/checksum inventory。操作者必须在工具外逐项盘点并
处置这些 artifact；manifest 只绑定完整手填的目标 runtime 和固定 discard decision，不能被描述成机器验证过的
旧文件清单。当前 `inspect` 自动安全打开并 hash encrypted dump、age identity 与 release archive；隔离恢复库和
全新 target 目前只做 strict typed 静态声明校验，不连接、不恢复也不探测，未来 apply 必须在写入前完成运行时
验证。旧 Redis 和 live 旧站不在检查范围。未知 key、缺 key、placeholder 或 target provenance 缺失继续阻断。

单一人工 legacy manifest 派生两个 `configuration_source=file_only`、`configuration_scope=boot_only` 启动文档：API 固定
`/var/lib/v2board/api/config.json`，worker 固定 `/var/lib/v2board/worker/config.json`；显式 `null`/空 array
也必须保留。API map 只含 API PostgreSQL URL、worker 非秘密 role 名和 Redis；worker map 只含 worker
PostgreSQL URL、API 非秘密 role 名、Redis 与 ClickHouse writer，不得出现 API URL 或 reader。错配
`runtime_role`、额外 secret key 和值型环境覆盖全部 fail closed。

两个角色文档不是长期的动态配置双写目标。它们只保存互相隔离的 datastore、`APP_KEY`、监听与网络策略等
启动材料；runtime path 由固定 systemd unit 提供，不进入角色 JSON，也不包含 operator baseline。
lifecycle 在 installation 行建立后、服务启动前，把 API/Worker 完整 typed
解析所得且逐值相同的规范化 candidate 直接加密写入 PostgreSQL authority；不产生任何临时或长期 seed
文件。首次写入要求 authority 完全为空；已有错值、孤儿 revision 或 state 漂移一律阻断。legacy v5 若在
authority 启用前崩溃，则清理未激活 target 并从同一 dump 重跑，而不是续接旧 checkpoint。API 启动只加载
authority，不能从 role file 自动建权威源。
Worker 缺少 active revision、无法解密、HMAC/typed 校验失败时不 ready，不能退回 boot 文件默认值
继续执行业务。管理后台保存后，API 与 Worker 都只消费 PostgreSQL 的同一 active revision。
native authority 已提交且两个角色均 ready 后，forward-only 路径必须安全删除本 operation 留下的
`.previous`/`.absent`/`.tmp` role-config artifact、fsync 两个父目录，并把清理结果绑定进 journal stage proof；
成功态不得遗留可能含旧完整 plaintext 配置的 rollback 文件。

operator revision 行不可 UPDATE/DELETE；公开 JSON 明确排除 `server_token`、`email_password`、
`telegram_bot_token`、`recaptcha_key`，这四项以 `APP_KEY` 派生 key 做 AES-256-GCM。完整 candidate 另有
domain-separated HMAC，nonce/AAD 绑定 installation、revision identity 与公开配置。保存顺序固定为：从
当前 typed snapshot 合成完整 candidate → 全量字段/跨字段校验 → 加密 → revision insert 与 active pointer
CAS 同事务提交 → API ArcSwap 应用并写自己的 applied ack → 返回现有 `{data:true}`。事务前失败不改变
active revision；active commit 后的 ack 短暂失败由 reloader 补写，不能把已提交配置谎报成保存失败。

API 仅能 INSERT revision、推进 state 并写 API ack；Worker 只能 SELECT revision/state 并写 Worker ack；
双方只能读对方 ack。`secure_path` 是明确支持的热更新：API 在 save 返回前切换当前 snapshot，动态 fallback
立即接受新 outer path，前端随后同源 `location.replace`，不能先落盘再等重启突然切换。listener、pool、
datastore credential、runtime path 等 boot-bound 字段不进入 operator save，仍需 lifecycle 维护窗口。

`validate/inspect` 不写文件；production gate 解除且未来 cold-import executor 完整接线后，它必须分别用
`0600` temp、fsync 和 atomic rename 写入。
生产部署固定使用不同的 `v2board-api`/`v2board-worker` Unix 用户和两个 `0700` 可写父目录，不能共享
config 目录；`V2BOARD_CONFIG_PATH` 由各自 systemd unit 固定，不能绕过 role 校验。

生产 datastore 分权固定为：

- PostgreSQL bootstrap、migration、API、worker principals；API/worker 连接同一 target，但 username
  不同，bootstrap/migration 不进入长期 runtime；
- ClickHouse bootstrap、schema、仅具 raw/按批次日聚合 INSERT + 批次核对 SELECT 的 relay writer、select-only
  analytics reader；bootstrap/schema 只进入一次性 job。当前没有 ClickHouse reader consumer，因此
  reader 不进入 API/worker runtime；worker 只持有 writer；
- Redis 使用独立 TLS credential/namespace。

`APP_KEY`、database/Redis/ClickHouse/payment credentials、trusted proxy/CORS 和 runtime/rules/frontend
path 是 boot/lifecycle F1；SMTP/reCAPTCHA/Telegram/server credential、secure path 与 app/subscribe URL
是 PostgreSQL operator F1。两类都必须有消费者清单、外部重注册、fingerprint、回滚和审计；前一类需要
maintenance 或显式 old/new bridge，后一类必须使用上述 revision/ack 协议。报告/日志/diff/argv 只能使用
secret manager version 或独立 audit key HMAC，不能输出明文或低熵裸 hash。

`/var/lib/v2board` 中各自可见的 config、`rules/custom.*` 和 lifecycle state 属于安装状态；生产使用
具名无登录用户而不是固定容器 UID。path/ownership 变更先在副本验证再原子切换，不能因不可写而创建
空替代目录。

旧 theme、Ant/Bootstrap/OneUI、custom CSS/JS 和打包 bundle 不进入新版 runtime。它们先由 operator 在
lifecycle 工具之外建立 F2 artifact inventory；当前 manifest 只绑定 discard decision，不扫描任意旧路径、
不生成这些文件的 checksum receipt。只有明确接受损失且旧回滚窗口结束后才可降为 F3。受支持 custom
subscription rules 必须严格 parse、原子安装并验证输出；不能盲拷模板或静默换 embedded default。

## 9. Worker、fencing、health 与部署

同一业务 task 在 native 升级前后最多产生一次结果。已应答 traffic/payment/mail/mutation 必须已有 durable
recovery path；native pending/leased work 不得被 cleanup 或升级删除。legacy v5 是具名破坏性例外：旧
Redis queue/retry/lease 与 MySQL `failed_jobs` 全部丢弃，不为这些旧任务承诺 exactly-once。worker 遇到错误
installation、过新/过旧 payload 或 epoch 必须 fail closed。

native upgrade 的有效 writer fence 必须由旧 writer 无法绕过的数据库权限、credential、deployment epoch
或 network policy 实施。看到进程退出、Redis lock 消失或当前无请求不够；proof 必须覆盖旧主机恢复网络和
process manager 自动拉起。legacy v5 不接收 live source credential，也不机器执行 fence：操作者在工具外
完全停止旧站后才创建 encrypted dump，dump 的不可变 SHA-256 是唯一 source cut。

schema/config migration 只由独立、串行、一次性 lifecycle job 执行，普通 API/worker startup 不自动迁移。
legacy 初始迁移顺序固定为：工具外停止旧站并生成 age-encrypted MySQL dump → 校验 archive/release/空 target
→ operation-owned 隔离恢复 → offline bulk conversion → verify → 启用 API/worker。旧 Redis 与节点不排空、
不审计、不复制；target 节点保持为空并在迁移后人工重建。它不存在 reporter 激活协议、bridge 或渐进放量。
未来 native 破坏性升级若确实需要兼容桥接，必须作为另一份升级计划单独获批。

PostgreSQL 不可用时核心 API/worker not ready；Redis 不可用时 session/lease 功能按 fail-closed 契约
降级。ClickHouse 短暂不可用时核心事务继续，analytics 标记 stale/unavailable，outbox lag 告警；不得
用对 PostgreSQL 的无界分析扫描伪装 fallback。长期故障在容量/背压门禁完成前是发布 blocker，不能
宣称核心可无限期继续。

frontend hashed asset 在受支持 rollout/rollback 窗口内不可变并保持可取；旧 HTML 引用的 entry、chunk、
CSS font/image 经过负载均衡命中新服务器/实例仍须成功。不能仅靠单机 `previous` symlink 证明跨实例 retention。

## 10. Backup、恢复和回滚

同一 fence 下的一致恢复集至少包含：

- PostgreSQL snapshot/PITR/WAL coordinate、sequence、migration/operation ledger；
- ClickHouse schema ledger、target generation、outbox replay window、replicated backup 或不可变归档；
- runtime config、custom rules、installation state、secret version/HMAC fingerprint；
- native 恢复所需 Redis namespace/type/TTL inventory；legacy v5 只要求新 target Redis 为空，旧 Redis
  全部丢弃且不做 inventory；
- API/worker native artifact digest、build manifest、frontend release/assets；
- payment/mail/Telegram/node 在维护窗口内的 replay/reconciliation 起点。

backup 加密、位于不同 failure domain，具备 RPO/RTO、retention 与访问审计。restore drill 必须在隔离、
禁用 worker/mail/payment/Telegram/node outbound 的环境真实执行。审计副本不得随业务 snapshot 一起回退。

接纳新写入前且 writer fence 未解除，可以恢复同一 operation 的一致恢复集。接纳新写入后禁止直接恢复
旧 snapshot；必须 PITR、durable mutation journal 或 forward recovery，保留新写入并单调 reconcile
session/traffic/credential epoch。无法证明不丢写入、不复活 credential 时，只能 forward recover。

ClickHouse 可整库/分区重建，但只有 PostgreSQL outbox/归档或另一份经过 restore drill 的恢复来源仍在
时才能宣称可重建。ClickHouse rollback 不得回滚 PostgreSQL authority。

## 11. 未来 native 破坏性升级固定协议

本节只适用于已经运行 PostgreSQL/ClickHouse 新版之后的未来升级，不适用于本次 legacy 初始迁移，也
不能据此给 MySQL 建设长期 bridge、双写或回退 runtime。

删表/列、收紧 type/NULL、改变单位/enum/key/URL/event identity/partition/order/TTL、替换 secret、修改
external payload 或停止旧 writer 支持，必须完整执行：

1. **Inspect**：零写入分类来源、规模、consumer 和 drift。
2. **Plan**：输出 redacted 步骤、impact、compatibility、space/lock、backup/rollback。
3. **Journal**：第一笔 mutation 前持久化 pending operation 和 plan/report/backup binding。
4. **Backup + restore proof**：一致恢复集在隔离环境真实验证。
5. **Expand**：先添加兼容 schema/endpoint。
6. **Bridge**：双读/写或明确旧 writer bridge，并持续一致性检查。
7. **Backfill**：小批、限速、可暂停、可 resume，每批 checkpoint。
8. **Verify**：ID、关系、聚合、event content、external behavior 和 drift 全部通过。
9. **Cutover**：显式切换 reader/writer/feature flag。
10. **Observe**：覆盖声明的 rollback 与 F2 consumer 窗口。
11. **Contract**：证明低版本 writer/consumer 已归零并单独获批后才 drop/retire。
12. **Final verify**：推进 minimum reader/writer 与 epochs，记录 completed。

任一步失败必须停在可识别状态。down migration 不能代替一致恢复集；破坏性升级不得在普通 systemd 启动
路径自动执行。ClickHouse retention 缩短必须先展示会删除的 partition/time range 并二次确认。

已安装的 analytics admission policy 与 installation UUID、policy SHA-256 不可变。未来若容量基准或
recovery/soft/hard 语义必须改变，必须作为具名破坏性 lifecycle/schema 版本：先展示旧/新阈值与当前
精确 rows/age/relation/database/headroom，再按上述协议安装新 policy generation。普通 runtime config
reload 或直接 `UPDATE` 永远不能改变安全水位。

## 12. Completion proof obligations

声明 lifecycle 完成至少需要机器可读、无 secret 的证据。下列通用项按路径适用；legacy v5 不因具名
丢弃项而伪造 source drain 或 provider 处置证明：

- 来源分类、pinned profile、schema/ledger checksum、PostgreSQL 18/ClickHouse 26.3/Redis capability；
- target database/principal 空状态、least privilege、TLS、`pg_hba`/network policy 和 installation binding；
- 所有保留业务表的 row count、完整 PK/natural-key set、canonical row hash、关系、sequence、NULL/zero/sentinel；
- user token/uuid/password/role/epoch；保留 order/payment/callback/reconciliation；金额以
  integer/decimal、流量以 exact integer/rate/rounding 比较，不经过 float；
- legacy encrypted dump、age identity、release archive、isolated restore 与 target 的路径/摘要/identity 绑定；
- legacy 固定损失结果：不连接旧 Redis，target Redis 为空；Stripe 配置与 status 0/1 Stripe 订单缺席，
  status 2/3/4 Stripe 历史的 `payment_id/callback_no` 为 NULL；非 Stripe payment/unfinished order 和
  `v2_user.balance` 原值保留；节点、流量明细、operational logs 与 failed jobs 缺席；
- PostgreSQL outbox → ClickHouse batch → acknowledgement、lost ACK、retry、partial/conflict quarantine、
  replay 与 ClickHouse outage；
- 完整 typed effective config、provenance、secret HMAC、runtime atomic write 和 custom rule rendering；
- API route/method/encoding/status/envelope、auth/logout、subscriptions、每个保留 payment provider、Telegram、
  node token/idempotency、邮件、Crisp/Tawk 与旧 frontend asset；
- native writer fence、backup restore、code/forward rollback、API/worker/schema/config/release compatibility；
- legacy 永久记录 encrypted dump SHA-256、release binding、target installation 和转换报告；PostgreSQL 是
  唯一事务 authority，ClickHouse 是分析投影。旧站永久退役由操作者负责，不用旧凭据可达性、旧 systemd
  状态或 Redis receipt 冒充 importer completion proof；
- native DDL/DML/backfill 每个 checkpoint 的 fault injection、安全 recovery 和容量/lock/lag budget；legacy
  则验证 pre-activation 清理以及从同一 dump 使用新 operation ID 完整重跑。

只有 implemented、tested、restore-drilled 且 proof 归档，路径才可标记 supported。一次人工成功、口头
确认或文档完成都不提升状态。

## 13. F3 与禁止的销毁动作

F3 仅包括 dependency/build/test/cache/report output、worker health timestamp、真正可重算且不承担
idempotency/session/security/recovery 的 metrics/cache，以及在 inventory、owner acceptance 和一次性迁移
验收归档后退役的旧 runtime/theme/bundle artifact。

以下通常不是 F3：source 业务数据、PostgreSQL/ClickHouse migration ledger、installation/operation ledger、
runtime config、custom rules、payment verification history、native session、idempotency/outbox、pending/
leased work、reconciliation、ClickHouse 唯一恢复副本和仍被旧 HTML 引用的 assets。legacy v5 的完整具名
例外是：旧 Redis 全部状态、MySQL `failed_jobs`、Stripe payment 配置与 status 0/1 Stripe 订单、旧节点/
route/credential、`v2_stat_user`、`v2_stat_server`、`v2_log`、`v2_mail_log`、旧 runtime/theme/custom rules。
`v2_stat`、非 Stripe payment 配置与未完成订单、terminal Stripe 订单业务历史以及用户现有字段仍保留；
`v2_user.balance` 不退款、不补偿、不重算。

`make reset`、`docker compose down -v`、`FLUSHDB`、删除 runtime state directory、删除
PostgreSQL/ClickHouse 原生 data directory 或备份都是销毁操作，绝不能进入安装、迁移或普通升级路径。

## 14. 当前实现符合性

| 能力 | 当前状态 | 生产结论 |
| --- | --- | --- |
| lifecycle JSON v5 loader | 只严格解析唯一 archive-first legacy v5；fresh/native v3 与退役 shape 都拒绝 | 只证明 spec 静态有效，不自动解除 production blocker |
| fresh `inspect` | 未实现；CLI 对 fresh v3 失败关闭 | bootstrap、journal、config promote、backup/recovery/apply 均未实现 |
| legacy `inspect` | 校验 encrypted dump/age identity/release 的文件、摘要和 archive 契约；restore/target 仅静态声明，不连接 live 旧 MySQL/Redis/target | archive-first converter、live target prerequisite、cleanup/restart 与最终安全审计完成前固定 blocked |
| native `inspect` | 未实现；CLI 对 native v3 失败关闭 | installation/build/schema machine binding、dry-run、impact estimator、journal/rollback/apply 均未实现 |
| PostgreSQL native baseline/runtime | 新 lineage 和 runtime 已迁移到 PostgreSQL | 只能服务已确认 native database；不等于 lifecycle apply |
| typed PostgreSQL outbox/ClickHouse projection | exact batch、26.3 lineage、TTL、normal/soft/hard admission 与真实 PG→CH integration 已建立 | 仍需基础设施容量/WAL 告警；HA 是单独部署选择 |
| runtime config authority | v5 语义验证 API/worker 两个最小 boot-only file map；native runtime 已支持加密不可变 revision、同一 active revision 与角色独立 ack | role file 安装与首次 authority seed 只由未来 production executor 在 gate 解除后执行；后台动态保存不写 role file；native upgrade executor 仍未实现 |
| lifecycle `apply` | legacy 单次 cold-import apply 受 typed production capability 控制；没有 authorize/resume；fresh/native 尚无 executor | 当前不支持任何生产写入；不得手工绕过 gate |
| legacy archive/restore/converter | v5 contract 固定 encrypted dump → isolated restore → retained-row conversion → pre-activation cleanup/restart；旧 Redis、Stripe 活跃状态和其他具名项按 manifest 丢弃 | 新流程的真实 MySQL 8.0/8.4 dump integration、结果校验、完整清理重跑与最终安全审计完成前不得启用 |

## 15. 代码与文档入口

- 数据库架构：[`docs/postgresql-clickhouse-invariants.md`](postgresql-clickhouse-invariants.md)
- legacy v5 指南：[`docs/legacy-migration-manifest.md`](legacy-migration-manifest.md)
- lifecycle 示例：[`fresh v3`](examples/fresh-install.v3.example.json)、
  [`legacy v5`](examples/legacy-migration.v5.example.json)、[`native v3`](examples/native-upgrade.v3.example.json)
- lifecycle grammar：[`backend/rust/crates/lifecycle/src/cli.rs`](../backend/rust/crates/lifecycle/src/cli.rs)
- legacy v5 spec/preflight：[`backend/rust/crates/provision`](../backend/rust/crates/provision)
- PostgreSQL migrations：[`backend/rust/migrations-postgres`](../backend/rust/migrations-postgres)
- ClickHouse migrations：[`backend/rust/clickhouse-migrations`](../backend/rust/clickhouse-migrations)
- analytics outbox/projection：[`backend/rust/crates/analytics`](../backend/rust/crates/analytics)
- runtime config：[`backend/rust/crates/config/src/lib.rs`](../backend/rust/crates/config/src/lib.rs)
- 唯一旧版 schema：[`references/wyx2685-v2board/database/install.sql`](../references/wyx2685-v2board/database/install.sql)
- HTTP 与前端 contract：[`backend/rust/crates/api/src/routes.rs`](../backend/rust/crates/api/src/routes.rs)、
  [`frontend/packages/api-client/src`](../frontend/packages/api-client/src)、
  [`frontend/tests/lib/interaction-scenarios.mjs`](../frontend/tests/lib/interaction-scenarios.mjs)

## 16. 变更控制

1. 所有者复查后，未明确修改的条目继续冻结。
2. 降低保护等级、缩短兼容/retention 或新增可丢弃项必须单独提交，说明消费者、数据风险、恢复影响和
   替代方案。
3. 实现与本文冲突时默认实现有缺陷，不能以当前代码行为反向降低要求。
4. 新增 external contract、持久化字段/event、secret、Redis key、worker 或 runtime file 时，在同一变更
   更新本契约、保护等级和 proof obligation。
5. F0/F1 尽可能使用独立 contract/integration/fault-injection test；文档与手工检查不能替代自动证据。
