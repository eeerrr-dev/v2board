# MySQL 一次性导入不可变契约

状态：**首个 native 版本发布前，只保留 `mysql-import.v1` 这一条旧数据导入路径。**

新版尚未发布，也没有任何已经安装的 native 数据库需要兼容。当前 PostgreSQL、
ClickHouse schema、导入清单和导入代码都是首发前基线，可以直接整理、合并或重写；
不得为了本地提交历史增加兼容 migration、旧 schema 分支或升级桥接。

本文只定义从旧 MySQL 导入新版的固定契约。新版正常运行后的 PostgreSQL backup/PITR、
ClickHouse replay 和 frontend `current`/`previous` 发布语义属于日常运维，不是本次导入流程。

## 1. 第一性流程

唯一流程固定为：

1. 停止旧站全部写入；
2. 从旧 Oracle MySQL 8 导出一个完整 dump；
3. 在已停写的旧生产机上，把 dump 导入隔离的一次性 staging MySQL，仅供 converter 读取；
4. 将固定保留的数据确定性转换进全新的 PostgreSQL 18；
5. ClickHouse 26.3 从空的 native event history 开始，新 Redis 从空状态开始；
6. 根据清单中的显式目标值生成新的 API/worker 配置；
7. 验证数据、配置和服务启动条件，然后启动新版。

旧 MySQL 只负责生成 dump。导入器不修改旧库，不原地改表，不把旧库变成新版 runtime，
也不与它双写。staging MySQL 只是读取 dump 的临时转换输入，不是旧数据库恢复方案。

当前 converter 需要在导入期间运行一个临时 MySQL 8 engine，以便从已加载的 dump 读取旧 schema。
默认拓扑是在停止旧站写入后，由旧生产机运行 staging 和 converter；staging 必须是使用独立 data
directory/volume、端口、凭据且仅本机监听的第二个 instance，不能在原 MySQL instance 内新建
database，也不能挂载原 data directory。converter 使用临时 migration principal 从旧机写入新
PostgreSQL。旧机容量不足时才使用一次性迁移 VM。新生产机不运行 MySQL，staging 在成功或失败后
连同临时数据一起删除。

导入器没有在线迁移、渐进切流或新旧并行运行路径，也不保存可以续接的中间状态。一次导入
失败后，丢弃这次产生的不完整 staging 和新 target，修正问题，再从同一个 dump 向新的空
target 运行一次。旧 MySQL 从始至终没有被改变。

## 2. 唯一输入与 target

`mysql-import.v1` 清单只包含三部分：

- `source`：完整 MySQL dump 的绝对路径、SHA-256，以及一次性 staging MySQL URL；
- `target`：全新的 PostgreSQL、ClickHouse、Redis 和两个配置文件路径；
- `runtime`：新版 API/worker 所需的完整显式配置值。

示例见 [`mysql-import.v1.example.json`](examples/mysql-import.v1.example.json)。清单不存在
kind 分支、可配置的保留/丢弃策略、发布归档、旧 Redis URL、旧 MySQL live URL、旧 systemd
unit 或 Stripe credential。固定数据策略属于 converter 代码和本文，操作者不能逐次修改。

输入和 target 必须满足：

- dump 来自完全停写后的 Oracle MySQL 8.0/8.4，并以小写 SHA-256 绑定；
- dump 文件为受限权限的 regular file，不是 symlink；
- staging MySQL 是旧生产机上新建、隔离、仅本机可达的一次性第二 instance/database，只允许
  converter 访问；若旧机容量不足则使用一次性迁移 VM；
- PostgreSQL target 是全新的空数据库；
- ClickHouse target 没有旧事件；
- 新 Redis 为空，不能通过读取或清空旧 Redis 来制造 target；
- API、worker、migration 和 datastore principal 按目标架构分离。

MySQL 5.7、Percona、MariaDB、兼容代理或无法匹配固定旧 schema 的 dump 不在支持范围内。

## 3. 固定保留边界

旧 MySQL source 名是 dump 的真实契约，继续保留 `v2_*`；新 PostgreSQL target 是尚未发布的首发
schema，从一开始就不带该前缀。为避开 PostgreSQL 关键字，用户表和订单表使用复数。固定映射为：

| 旧 MySQL source | 新 PostgreSQL target | 行级规则 |
|---|---|---|
| `v2_server_group` | `server_group` | 全部保留 |
| `v2_plan` | `plan` | 全部保留 |
| `v2_payment` | `payment_method` | 排除 Stripe 配置 |
| `v2_coupon` | `coupon` | 全部保留 |
| `v2_user` | `users` | 全部保留 |
| `v2_order` | `orders` | 按第 4 节处理 Stripe 行 |
| `v2_commission_log` | `commission_log` | 全部保留 |
| `v2_invite_code` | `invite_code` | 全部保留 |
| `v2_giftcard` | `gift_card` | 全部保留 |
| `v2_knowledge` | `knowledge` | 全部保留 |
| `v2_notice` | `notice` | 全部保留 |
| `v2_ticket` | `ticket` | 全部保留 |
| `v2_ticket_message` | `ticket_message` | 全部保留 |
| `v2_stat` | `stat` | 全部保留 |

旧礼品卡使用记录转换为新的 `gift_card_redemption`。旧表没有兑换时间，因此这类派生行固定使用
`created_at=0` 和 `created_at_provenance=legacy_unknown`，不伪造历史时间。除此以外，所有保留行都
遵守以下规则：

- 主键、自然键、父子关系、状态和时间原值保留；
- `NULL`、0、空字符串和空集合不互相折叠；
- password hash、永久订阅 token、用户角色、套餐和封禁状态原值保留；
- source `v2_user.balance` 到 target `users.balance` 不退款、不补偿、不重算；
- MySQL 已落盘的 `u`、`d`、`transfer_enable` 原值保留；
- 金额继续使用整数 cents，流量继续使用整数 bytes；
- PostgreSQL sequence 必须高于已经导入的最大 ID；
- collation、唯一键或关系发生冲突时停止，不自动挑选、合并或修复数据。

## 4. Stripe 固定规则

导入器不联系 Stripe：不调用 API，不枚举、不取消、不结算、不删除，也不检查任何
provider-side object。Stripe 账户状态不是导入前置条件或完成证明。

本地 MySQL 行固定按以下方式处理：

- source `v2_payment.payment` 大小写不敏感地以 `stripe` 开头的配置全部丢弃；
- 绑定这些配置且 status 为 `0` 或 `1` 的 Stripe 订单全部丢弃；
- status 为 `2`、`3` 或 `4` 的 Stripe 订单只保留业务历史，目标中的 `payment_id`、
  `callback_no` 和派生 callback hash 为空；
- 非 Stripe payment 配置和非 Stripe 未完成订单按普通业务数据保留；
- Stripe 订单出现非 `0..4` status、损坏关系或无法判定 provider 时停止导入，不猜测处理。

上述规则不会触发退款、补单、重新结算或用户余额调整。

## 5. 固定丢弃边界

以下内容不进入新版：

- 整个旧 Redis：未落 MySQL 的尾部流量、queue/retry work、session、OTP/TOTP、临时订阅
  URL、cache、lock、lease、rate-limit、幂等临时状态和 Horizon metadata；
- MySQL `failed_jobs`；
- 旧 MySQL source `v2_log`、`v2_mail_log`；
- 旧 MySQL source `v2_stat_user`、`v2_stat_server`；
- 旧 MySQL source `v2_server_route`；
- 旧 MySQL source `v2_server_shadowsocks`、`v2_server_vmess`、`v2_server_trojan`、`v2_server_tuic`、
  `v2_server_hysteria`、`v2_server_vless`、`v2_server_anytls`、`v2_server_v2node`；
- 新 PostgreSQL target `server_credential` 保持为空；
- 旧 `.env`、PHP/Laravel 配置、theme、打包前端、custom CSS/JavaScript 和 operator script；
- 任何旧 ClickHouse event history。

因此 PostgreSQL 的 `system_log`、`mail_log`、`user_traffic`、`server_traffic`、`server_route`、
上述丢弃的协议 target（如 `server_shadowsocks`、`server_vmess`）和 `server_credential` 在导入结果中
都必须为空。这里的 `v2_*` 只指旧 MySQL source；不得把它重新用作 native target 名，也不得增加
rename migration、alias 或兼容 view。

导入器没有旧 Redis URL，也不会连接、扫描、排空、复制或检查旧 Redis。新版节点 inventory
为空，由操作者使用新的 `server_token` 手工重建仓库外节点。

## 6. 新配置

旧运行时文件不导入、不合并、不执行。操作者在 `runtime` 中填写新版所需值，导入器用同一份
typed candidate 生成：

- `/var/lib/v2board/api/config.json`
- `/var/lib/v2board/worker/config.json`

两个文件位于各自 Unix 用户的受限目录，权限为 `0600`，只包含该角色所需的 boot-only
字段和 datastore credential。动态 operator 配置写入 PostgreSQL configuration authority；
API 与 worker 必须能解析相同的规范化配置结果。

## 7. 完成验证

启动新版前至少验证：

- dump SHA-256 和固定旧 schema；
- PostgreSQL target 起始为空，转换后 schema 与 migration baseline 精确一致；
- 每张保留表的行数、主键集合、自然键、关系、关键值和 sequence；
- 固定丢弃表在 target 中没有旧行；
- Stripe 丢弃、保留和解绑结果完全符合第 4 节；
- source `v2_user` 与 target `users` 的 balance、永久 token 和 MySQL 已落盘流量逐值一致；
- ClickHouse 没有伪造的旧事件，新 Redis 为空；
- API/worker 配置通过各自完整 typed parser，并满足文件 owner/mode；
- API、worker、PostgreSQL、ClickHouse、Redis 和 frontend release 的启动检查通过。

单次 SQL 成功、进程启动或 `/readyz` 成功都不能替代数据结果验证。

## 8. 首发前基线

在第一个 native 生产版本明确发布以前：

- PostgreSQL 和 ClickHouse migrations 可以直接合并、重编号、重写或删除；
- 不为本地 Docker volume、未推送提交或旧清单格式保留兼容代码；
- 不提前创建尚不存在的安装、升级 schema、示例或 executor；
- 文档、代码、测试和示例只能描述 `mysql-import.v1`；
- 本地旧 volume 只是可丢弃开发产物，确认无需要数据后重新创建即可。

首发后若确实出现 native 安装，届时再基于真实发布版本单独设计升级契约；不得提前把推测性
升级系统塞回这次 MySQL 导入。

## 9. 当前实现边界

`validate` 和 `inspect` 只用于检查 `mysql-import.v1`、dump 文件和静态 target/runtime 声明。
当前没有生产写入命令或 executor。在真实 MySQL 8 dump → staging → PostgreSQL 转换、结果验证、
配置安装和启动检查完成端到端测试前，不能宣称已经可以执行生产导入，也不能用手工部分写入替代
缺失的 executor。
