# 旧版迁移清单 v5

本指南只适用于从 `references/wyx2685-v2board` 固定旧实现迁移到新版 PostgreSQL 18、
ClickHouse 26.3 和 Redis。唯一旧数据库格式是 Oracle MySQL 8.0/8.4；MySQL 5.7、
Percona、MariaDB 和兼容代理不在支持范围内。MySQL 不会成为新版 runtime 的可选数据库。

旧版迁移现在只有一个 schema：`schema_version: 5`。它是一条停机、破坏性、
archive-first 的冷导入路径，不提供旧 schema 兼容分支、在线迁移、CDC、双写、shadow read、
旧 MySQL 回滚或滚动切换。

完整的数据所有权见
[PostgreSQL + ClickHouse 持久化不变量](postgresql-clickhouse-invariants.md)，共同 lifecycle
边界见[安装、迁移与升级不可变契约](upgrade-invariants.md)。

## 设计目标

迁移器只消费两个不可变输入：

- 操作者在旧站完全停止后生成的 age 加密 MySQL dump；
- 已验证的 native release archive。

它不连接旧 Redis，不连接仍在服务的旧 MySQL，不接收旧 systemd unit、旧数据库 fence
凭据或旧 Redis prefix。旧站停机与 dump 创建是操作者在迁移器外完成的前置步骤；迁移器
不会用一组自报的 unit 名称假装证明旧站已经停止。

这一边界刻意牺牲旧运行时瞬态状态，换来更小、更容易复查的流程：失败发生在新版启用前时，
清理未激活 target 和隔离恢复库，然后从同一个不可变 dump 重新开始，不进行 checkpoint resume。

## 固定流程

从操作者视角只有四件事：停旧站并导出 MySQL、从 dump 恢复并转换 PostgreSQL、按 manifest 生成新配置、
验证后启动新版。旧 Redis 和 Stripe provider 都不形成旁路流程；没有 drain、对账、授权文件或断点续跑。

### 1. 停止旧站

进入维护窗口后，操作者必须停止所有旧写入来源，包括：

- PHP/API 与管理后台写入；
- scheduler、queue worker 和定时任务；
- 仍会向旧站上报流量的远端节点；
- 新订单创建和支付入口。

仓库不接收这些服务的 unit 名称，也不远程控制旧节点。是否已经停止由操作者负责，不能靠
manifest 中的布尔值冒充运行时证据。

### 2. 生成不可变加密 dump

在旧站停止后，使用受支持的 MySQL 8 `mysqldump` 导出完整旧库，立即用 age 加密，并计算
加密文件的 SHA-256。不要把明文 dump 长期写入迁移目录；如果导出过程产生了临时明文，完成
加密和摘要后必须安全清理。

加密 dump 是唯一旧数据输入和永久冷归档。它应当：

- 位于绝对路径；
- 是 owner-only 的 regular non-symlink file；
- 在 manifest 中绑定 64 位小写 SHA-256；
- 与 age identity 分开保存；
- 在迁移成功后仍按备份保留策略保存。

age identity 同样使用绝对路径和 SHA-256 绑定，但不与加密 dump 一起归档。示例将其放在
`/run/v2board-lifecycle-secrets/<operation-id>/age-identity`；生产中应由独立 secret source
提供，并限制为 owner-only。

### 3. 填写唯一 v5 manifest

从 [`legacy-migration.v5.example.json`](examples/legacy-migration.v5.example.json) 复制模板。
manifest 包含：

- 固定 `source.format=age_encrypted_mysql8_mysqldump`，以及 encrypted dump 路径、摘要和 age identity；
- 独立的隔离恢复 MySQL 8 URL；
- 全新的 PostgreSQL、ClickHouse 和 Redis target；
- 新 runtime 的完整目标值；
- 明确的数据丢弃与保留决策；
- native release ID、绝对 archive path 和 archive SHA-256；
- 固定的失败清理、一次性 activation 与 converter policy identity。

manifest 不含旧 MySQL URL、旧 Redis URL、Redis prefix、旧 systemd unit、source fence、
queue drain 或 source retirement 字段。旧字段会被 strict schema 当作 unknown field 拒绝，
不会静默忽略。

### 4. `validate` 与 `inspect`

`validate` 离线检查严格 JSON、必填字段、secret 独立性、绝对路径、摘要格式、runtime 类型和
固定损失决策。

`inspect` 不接触旧站。当前实现只安全打开并 hash 加密 dump、age identity 和 native release，验证
文件权限/inode/摘要及 release tree；它只校验 target 声明的静态 shape，不连接或探测 live target。
未来 apply 需要查看旧数据时，只能把同一加密 dump 解密到 manifest 指定的隔离 MySQL 8 恢复库，
然后从该恢复快照读取。隔离库不是 runtime，也不能接收用户流量。

### 5. `apply`

production gate 解除后，单次 `apply` 才能：

1. 重新验证 encrypted dump、identity、release 和空 target；
2. 在隔离 MySQL 中恢复 dump，并验证受支持的旧 schema；
3. 创建空 PostgreSQL 18、ClickHouse 26.3 和 Redis target；
4. 转换保留数据并验证关键值、关系和目标空表；
5. 从 manifest 生成 API/worker 两份 role-owned `0600` boot config；
6. 将动态 operator 配置写入 PostgreSQL authority；
7. 验证 API、worker 和前端 release 后，最后才启用新版 authority。

schema v5 没有 `authorize` 或 `resume` 阶段，也不生成可跨进程继续旧阶段的 authorization。
不要运行旧文档中的 `authorize`/`resume` 命令，也不要复用任何退役 schema 的 authorization、journal
或 receipt。

当前 production capability 仍然 fail-closed：`validate`/`inspect` 可用于准备和复查，`apply`
会在任何 target 写入前拒绝。检查通过不能被解释为生产迁移已经获准。

### 6. 失败与重跑

只要新版 authority 尚未启用，失败处理固定为：

1. 保留原 encrypted dump，不修改其字节；
2. 删除本次创建但尚未启用的 PostgreSQL database/roles、ClickHouse database/principals 和
   target Redis namespace；
3. 删除 operation-owned 隔离恢复库和临时文件；
4. 修复 converter、配置或基础设施；
5. 使用同一个 dump SHA-256 和一个新的 operation ID 从头运行。

这不是 rollback，也不是 resume。新版开始接收写入后，不允许重新开放旧 MySQL；后续故障按
native PostgreSQL/ClickHouse 备份恢复或 forward fix 处理。

## 明确接受的损失

### 旧 Redis：完全不检查、完全不导入

`decisions.legacy_redis` 固定为 `discard_all_without_inspection`。迁移器没有旧 Redis URL，
不会连接、SCAN、冻结、排空、复制、停止或探测旧 Redis。

该单一决定明确包含以下全部损失：

- 尚未写入 MySQL `v2_user.u/d` 的 pending upload/download traffic；
- Redis queue 中尚未执行的任务和 retryable failed work；
- Laravel session，所有用户和管理员切换后重新登录；
- OTP/TOTP 与验证状态；
- mode 1/2 临时订阅 URL 和双向映射；
- cache、lock、lease、rate-limit 和幂等临时状态；
- Horizon metadata、wake token 和其他 queue 展示状态。

永久订阅凭据 `v2_user.token` 位于 MySQL，会随用户数据保留。MySQL 已经落盘的 `u/d` 也会
保留；只有仍停留在 Redis 的尾部增量被牺牲。新 Redis 必须为空，从新版运行时状态重新开始。

MySQL `failed_jobs` 同样固定为 `discard`，不会复制或要求排空。

### Stripe：只丢本地 Stripe 状态，不访问 provider

`decisions.legacy_stripe` 固定为：

- `configuration: discard`：不复制 `v2_payment.payment` 大小写不敏感地以 `stripe` 开头的配置；
- `unfinished_orders: discard`：不复制仍处于未完成状态并绑定这些 Stripe 配置的订单；
- `provider_objects: ignore_uninspected`：不调用 Stripe API，不枚举、不取消、不结算也不删除
  PaymentIntent、Checkout Session、webhook 或其他 provider-side object。

因此 Stripe provider-side object 永远不是 migration blocker，但仓库也不会声称已经处置 Stripe
账户中的对象。旧库中 status 不在 0..=4、关系损坏或格式非法的 Stripe 本地行仍会 fail closed；
操作者若仍关心退款、争议或对账，必须在迁移流程之外自行处理。

非 Stripe payment 配置继续按数据库转换规则保留。非 Stripe 未完成订单不属于上述破坏性例外；
它们作为普通业务数据保留，只接受与其他保留行相同的关系、格式和目标唯一性校验，不因为
“未完成”本身成为迁移 blocker。已经终结的 Stripe 订单（status 2/3/4）继续作为业务历史保留，
但 target 中的 `payment_id` 和 `callback_no` 固定为 `NULL`，不会保留可再次调用 provider 的活跃绑定。
迁移不会为已丢弃的 Stripe 订单退款、补单或重新结算，`v2_user.balance` 始终按 MySQL 已落盘值
原样保留。

### 其他固定丢弃项

| 字段 | 固定值 | 结果 |
| --- | --- | --- |
| `legacy_runtime_files` | `rebuild_from_manifest` | 不读取旧 `.env`、Laravel/PHP 配置或运行时文件；由新 manifest 生成配置 |
| `failed_jobs` | `discard` | 不复制 MySQL `failed_jobs` 或旧 queue 状态 |
| `nodes` | `discard_and_manual_rebuild` | 不复制旧节点、路由和 credential；新版从空节点 inventory 开始 |
| `legacy_traffic_details` | `discard` | 不复制 `v2_stat_user`、`v2_stat_server` |
| `legacy_operational_logs` | `discard` | 不复制 `v2_log`、`v2_mail_log` |
| `legacy_theme` | `discard` | 不导入旧主题、打包资产、CSS 或 JavaScript |
| `legacy_custom_rules` | `none` | 不执行或猜测旧 operator 脚本与规则 |

为防止“只写丢弃、不说明核心数据是否保留”，strict v5 还要求四项固定正向声明：

- `mysql_business_data=preserve`；
- `mysql_persisted_user_traffic=preserve`；
- `permanent_subscription_tokens=preserve`；
- `non_stripe_payment_configuration=preserve`。

当前 strict schema 的 `legacy_custom_rules` 只接受 `none`；如未来允许另一策略，必须提升具名 policy，不能
在现有 v5 中静默改变。

旧节点必须由操作者在导出前停止，迁移后使用新 `server_token` 手工重建。仓库不迁移或激活
V2bX、XrayR 等仓库外节点程序。

## 数据保留边界

v5 从恢复快照复制并验证以下核心业务表：

- `v2_server_group`
- `v2_plan`
- `v2_payment`，但排除 Stripe 配置
- `v2_coupon`
- `v2_user`
- `v2_order`，但排除未完成 Stripe 订单
- `v2_commission_log`
- `v2_invite_code`
- `v2_giftcard`
- `v2_knowledge`
- `v2_notice`
- `v2_ticket`
- `v2_ticket_message`
- `v2_stat`

并从旧礼品卡使用记录派生 `v2_giftcard_redemption`。用户 ID、邮箱、密码 hash、余额、套餐、
永久 token、MySQL 已落盘 `u/d`、quota、订单金额和已保留支付配置必须满足 converter 的值验证。

以下表保留在 encrypted dump 中，但不进入新 runtime：

- `v2_log`
- `v2_mail_log`
- `v2_stat_server`
- `v2_stat_user`
- `v2_server_route`
- `v2_server_shadowsocks`
- `v2_server_vmess`
- `v2_server_trojan`
- `v2_server_tuic`
- `v2_server_hysteria`
- `v2_server_vless`
- `v2_server_anytls`
- `v2_server_v2node`

派生的 `v2_server_credential` 同样不创建。旧 ClickHouse 历史不会生成；新版 ClickHouse 从空的
native event epoch 开始，只保存由 PostgreSQL outbox 投影、可重建的分析事实。

被放弃的数据仍存在于 immutable encrypted dump 中。archive SHA-256 是整个旧库的来源身份；
无需为了证明“没有复制”而再次为每张明确放弃的表建立 live-source 指纹或旧 Redis receipt。

## Runtime 与 target

工具不会导入、合并、执行或推断旧 `.env`、`config/v2board.php`、theme、PHP、CSS 或
JavaScript。`runtime` 的每个目标值都由操作者填写。`legacy_runtime_files=rebuild_from_manifest`
表示重建配置来源，不表示丢弃 MySQL 中的业务配置表。

manifest 派生：

- `/var/lib/v2board/api/config.json`
- `/var/lib/v2board/worker/config.json`

两份文件属于不同 Unix role、权限为 `0600`，只携带各自 boot-only 凭据。动态 operator 配置
由 migration principal 写入加密的 PostgreSQL configuration authority；API/worker 必须加载同一
active revision 并分别 ACK。

### PostgreSQL 18

操作者填写 bootstrap、migration、API、worker 四个不同 principal。后三者指向同一目标库，
`require_database_absent=true`、`require_roles_absent=true`。生产连接使用 `sslmode=verify-full`，
collation/ctype 固定为 `C.UTF-8`，`pg_hba.conf` 和网络策略由外部基础设施管理并提供证据。

### ClickHouse 26.3

操作者填写 bootstrap、schema、writer、reader 四个不同 principal。`require_database_absent=true`、
`require_principals_absent=true`，初始拓扑为 standalone non-replicated。raw/aggregate retention 与
analytics admission 容量边界必须显式填写。旧统计不会伪造成 ClickHouse event。

### Redis

这里只指**新 Redis**。目标 URL 使用 `rediss://`，且 `require_empty_redis=true`。工具不会恢复旧
RDB/AOF，也不会用 `FLUSHDB` 帮忙清空 target。target 还固定声明
`api_runtime_config_path=/var/lib/v2board/api/config.json` 和
`worker_runtime_config_path=/var/lib/v2board/worker/config.json`。

## Archive 与隔离恢复契约

`source.encrypted_mysql_dump_path` 和 `source.age_identity_path` 必须是规范化绝对路径，不能是
symlink。二者摘要、加密 dump 的最大字节数和命令总 timeout 都进入 manifest binding。

`isolated_restore_database_url` 使用独立 MySQL 8 credential，不能指向 PostgreSQL target 或任何
生产 runtime 数据库。恢复库必须不存在或为空；迁移器只能创建、读取、验证和删除本 operation
拥有的隔离库。每次 converter 读取前都应重新验证 encrypted dump SHA-256 和恢复结果。

`maximum_encrypted_dump_bytes` 是流式硬上限，`command_timeout_seconds` 是整个解密/恢复动作的
边界。示例的 256 GiB 和 24 小时只是占位起点，必须按实际 dump 大小与基础设施速度复核。

release archive 由 `execution.release.release_id`、绝对 `archive_path` 和 `archive_sha256` 唯一绑定。
`release_id` 必须是 1..=128 字节，只能包含 ASCII 字母、数字、`.`、`_`、`-`，不能是 `.` 或 `..`，
也不能包含校验器拒绝的 placeholder marker。
archive 文件必须是 `root:root` 且权限精确为 `0400`；它不能是 symlink，也不能在检查期间被替换。
它仍必须满足 native release 的完整文件树、内部 checksum、systemd 和前端 `current`/`previous` 契约；
这与旧数据 dump 是两个不同输入。

`execution` 还必须逐字绑定：

- `failure_policy=wipe_unactivated_target_and_restart_from_same_dump`；
- `activation_policy=activate_once_after_full_verification`；
- `converter_policy_marker=v2board.cold-import.v1:discard-stripe-payments;discard-unfinished-stripe-orders;scrub-retained-stripe-order-bindings`。

marker 是 converter 的版本化策略身份，防止 manifest 的损失声明与实际 Stripe 行转换算法漂移。

## 使用示例

模板含公开 placeholder，不能直接运行：

```bash
cp docs/examples/legacy-migration.v5.example.json /secure/private/legacy-migration.json
chmod 600 /secure/private/legacy-migration.json
sudo chown root:root /secure/private/v2board/v2board-native-linux-amd64.tar.gz
sudo chmod 0400 /secure/private/v2board/v2board-native-linux-amd64.tar.gz

sudo v2board-lifecycle validate --manifest /secure/private/legacy-migration.json
sudo v2board-lifecycle inspect --manifest /secure/private/legacy-migration.json
```

manifest 必须是 1 byte 到 2 MiB 的 regular non-symlink file，并且不能授予 group/world 权限。
最终命令以 root 运行，才能读取精确为 `root:root 0400` 的 release archive；不要为了让普通用户
执行检查而放宽 archive 权限。
URL 中的特殊字符需要 percent-encode；校验器会先严格解码 credential 与 database path 再做
placeholder、独立性和标识符校验。PostgreSQL principal/database 解码后必须匹配
`[a-z_][a-z0-9_]*`，且不超过 63 字节。报告不得输出明文 secret。

当前不要运行：

```text
v2board-lifecycle apply ...       # production capability 尚未开放
v2board-lifecycle authorize ...   # schema v5 不存在该阶段
v2board-lifecycle resume ...      # cold import 只允许清理后从头重跑
```

## 当前门禁与成熟度

当前能够安全使用的是 strict manifest validation，以及 encrypted dump/age identity/release 的只读
inspection。live target prerequisite、隔离恢复、转换、配置安装、activation 与 cleanup 都还不是已开放的
production apply 能力。production `apply` 必须继续 fail-closed，直到新的 archive-first 流程完成：

- Oracle MySQL 8.0/8.4 encrypted dump → isolated restore → PostgreSQL conversion 集成验证；
- Stripe/Redis/failed-job 丢弃策略的负面与结果测试；
- 未激活 target 的完整清理与同一 dump 重跑测试；
- native Linux/systemd 部署和最终安全审计。

已删除的旧在线 Redis 与源端控制故障矩阵不再属于产品契约，不能作为新流程成熟度证据。最终评价
必须以唯一 v5 archive-first 路径的实际测试为准，而不是沿用已删除流程的测试数量。
