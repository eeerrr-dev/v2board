# 安装、旧版迁移与升级不可变契约

状态：**冻结；所有者已确认手填 v2 清单、target MySQL 自动创建意图与两阶段确认流程**

唯一旧版来源：`references/wyx2685-v2board`

唯一旧版 commit：`7e77de9f4873b317157490529f7be7d6f8a62421`

本文定义全新安装、从唯一旧版迁移、原生版本升级和未来破坏性升级共同遵守的边界。
在所有者明确修改本文前，安装器、迁移器、升级器、回滚工具和发布流程都必须按本文执行。

**本文是目标规范，不代表当前实现已经满足。** 当前 CLI 除 native `migrate` 和重置已有管理员密码外，
实现了 `provision validate` 的严格 lifecycle JSON 文件校验、`provision inspect` 的在线有界只读检查，
以及 `provision plan` 的 fence 后最终有界只读检查。它们都不写 MySQL、Redis 或 runtime config；当前
明确为 `apply_available=false`，不存在 `provision apply`，
也没有 fresh-install/legacy bridge/operation journal/config materialize/backup/restore/cutover/release-manifest
全流程。未实现的静态能力会列入 `implementation_blockers`，所以当前 inspect/plan 均安全阻断；只有
未来关闭这些缺口并启用 apply 后，`compatible` / `ready_for_confirmation` 才表示**各自检查阶段**没有
blocker，仍不是迁移已经完成。`report_sha256` 是其自身字段置空时 canonical report payload 的摘要，
包含独立审计密钥对原始 manifest bytes 的 HMAC binding 和现场实例 identity；它并非最终打印 JSON
文件的直接哈希，在 operation journal、完整 mutation plan 与 backup proof 落地前也不能单独充当
apply/resume 的完整 lineage。
在对应能力、自动化 proof 和隔离 restore drill 全部通过前，不得把现有 `migrate`、`validate`、`plan`
或 README 手工步骤称为成熟的生产安装、旧版迁移或升级方案。

本文中的“不变”指**安装或升级流程不得静默改变**。用户主动修改密码、管理员显式封禁用户、
用户主动执行 `resetSecurity` 等正常业务动作不受禁止，但升级器不得伪装成这些业务动作。

## 1. 规范用语与保护等级

- **必须 / 不得**：硬性要求，不满足即必须停止安装、迁移、升级或切流。
- **应当**：除非有记录在案且经所有者批准的理由，否则必须执行。
- **可以**：不影响不可变契约时允许执行。

所有状态分为四级：

| 等级 | 定义 | 允许的变化方式 |
| --- | --- | --- |
| F0 — 保值与追加历史 | 身份、已有数据、历史、业务语义或永久外部契约 | 不得静默修改、删除、重解释或回退；只可追加审计历史、单调推进 epoch，或执行经证明语义等价的表示迁移 |
| F1 — 协调轮换 | 密钥、路径、域名、基础设施身份等可轮换状态 | 必须是独立、显式、可审计的轮换；必须有桥接、验证和回滚 |
| F2 — 有界兼容 | 旧协议、旧凭据或旧资源可在无受支持消费者后退役 | 必须有采用率/drain 证明；无法证明匿名消费者为零时，须有所有者批准的最大支持窗口、通知、明确截止点和失败策略 |
| F3 — 可重建或明确不迁移 | 缓存、构建产物等非权威状态，或已 inventory 且所有者明确接受损失的 unsupported artifact | 前者删除后必须能从 F0/F1 完整重建；后者必须已有具名接受记录且回滚窗口结束 |

以下默认规则永久生效：

1. **未分类即 F0**。任何新发现的表、列、文件、Redis key、配置项或外部行为，在本文明确降级前
   按 F0 处理。
2. **歧义即停止**。不得通过猜测、取第一条、自动删除、自动取消、自动补默认值等方式消除歧义。
3. **没有验证就没有完成**。SQL 执行成功、进程启动或 `/readyz` 返回成功，都不能单独证明升级完成。
4. **没有退役证明就不能删除兼容层**。优先证明无受支持消费者；匿名消费者无法归零时，必须走所有者
   批准的最大支持窗口并记录影响。单纯代码搜索、未批准的时间经过或“应该没人用了”不构成证明。
5. **升级不得夹带轮换**。应用版本升级不得同时自动轮换密钥、用户 token、节点凭据、域名或路径。
6. **配置默认值不得改变既有安装的有效行为**。新增版本可以为全新安装采用新默认值；既有安装必须
   物化旧有效值或通过版本化配置迁移明确改变。
7. **表示可以迁移，语义不能漂移**。表名、列名、数据类型、JSON 形状和存储位置可以通过受控迁移
   改变，但值、单位、状态含义、关联和外部可观察结果必须满足本文。

installation UUID、既有 ledger/operation 行和已完成记录是 append-only F0；schema/config/data epoch
只能经审计单调推进。minimum reader/writer 的历史是 F0，提高当前 minimum 是完成 F2 contract 的
显式动作。单个 release manifest 内容不可变；active release pointer 是受审计的 F1/F2 切换状态。

## 2. 支持的来源与 lineage

### 2.1 唯一旧版来源

旧版只指 pinned submodule commit
`7e77de9f4873b317157490529f7be7d6f8a62421`。不承诺兼容更早版本、其他 fork、其他 commit
或声称“也是最新版本”但无法命中指纹的安装。

生产镜像和运行时不得读取、挂载、执行、复制或回退到 reference。reference 只允许用于：

- 生成和复查旧版 schema/config/runtime 指纹；
- 构造旧版迁移 fixture；
- 审计外部行为契约；
- 人工只读比较。

reference commit 变化时，必须创建新的、具名的来源指纹和迁移适配器；不得静默修改现有适配器的
含义。

### 2.2 来源分类

任何写操作之前，生命周期工具必须只读分类：

| 分类 | 条件 | 允许操作 |
| --- | --- | --- |
| `empty` | 无业务表、无 native lineage、无残留安装状态 | 仅允许全新安装 |
| `legacy-reference-supported` | 代码 lineage 命中唯一 pinned commit、无 native lineage；schema 命中具名 compatibility profile；版本化 lifecycle spec 完整有效；旧 runtime artifact 均已 inventory 且有显式处置 | 仅允许唯一旧版迁移 |
| `legacy-reference-drift` | 可识别为 reference，但 schema 漂移、spec 无效或存在未确认的 runtime artifact | 禁止写入；输出完整差异 |
| `native` | installation identity、migration ledger、schema/config epoch 一致 | 仅允许原生升级 |
| `recoverable-pending` | installation identity 与已知 lifecycle plan 匹配，pending operation 的 source/target fingerprint、plan checksum、checkpoint 和 backup reference 均可验证 | 仅允许 resume、受控 rollback 或 recovery；不得开始新 operation |
| `dirty-or-unknown` | 空库夹杂表、半迁移、checksum 异常、来源不明等其他情况 | 禁止写入 |

分类不得只看表是否存在。旧版数据库指纹至少包含表、列顺序、类型、nullable、默认值、生成列、
主键、唯一键、普通索引、外键、check、字符集和 collation。MySQL server 版本、能力和 sql mode
必须单独检查。

唯一旧版迁移不读取或推断旧 `.env`、`config/v2board.php`、theme 和 `resources/rules/custom.*` 的值，
更不得 eval/include PHP；它们只做文件存在性和 checksum inventory。操作者在唯一、版本化、强类型
JSON lifecycle spec 中手填目标配置并为每个旧 artifact 选择保留、替代或放弃。缺少必填 key、未知 key、
placeholder secret 或未确认 artifact 都属于 drift；不得用新版默认值补齐。

当前 `provision plan` 尚未取得旧 runtime root，因此还不会执行上述旧文件 inventory；spec 中的
`none`、`discard_confirmed` 和 attestation 只是操作者声明，不是工具已经验证的事实。当前 schema
preflight 也只覆盖具名 core table 的有限 canonical 字段和少量数据/Redis blocker，尚不能单独证明
`legacy-reference-supported`，更不能证明 manifest 中自报的 reference commit 就是实际运行过的代码 lineage。

这里的 `supported` 要求代码来源和**必需兼容结构**精确命中，不要求数据库与全新 `install.sql` 字节级相同。
profile 必须显式列出允许存在/缺失的历史索引、已知残留表和等价 DDL；命中 allowlist 的残留不算
drift。不得用宽泛的 `IF EXISTS`/`IF NOT EXISTS` 或忽略错误来冒充兼容 profile。

旧版命中同一代码 commit 也不能跳过实际指纹。旧版更新命令会忽略单条 SQL 异常，真实数据库可能
处于部分更新状态；任何未在 profile 中解释、且可能影响业务数据或约束的差异必须归入
`legacy-reference-drift`。它只能先生成只读差异和单独修复计划，不能进入自动迁移。

### 2.3 Native 基线冻结点

本文假定当前真实生产来源只有上述旧版。如果已经存在任何不可丢弃的 native 数据库，该假定无效，
必须在重写或 squash migration 前先补录其 lineage。

- 首个 native 正式发布前，可以把开发期 schema 演进整理成一个 current native baseline，并把旧版
  转换保留为独立 legacy bridge。
- 首个 native 正式发布后，已发布 migration 的版本号、顺序、内容和 checksum 均为 F0：不得修改、
  重编号、删除或 squash，只能追加。
- 任何 migration 一旦被不可丢弃的数据库记录为成功，即视为已发布，不以 Git tag 是否存在为准。

## 3. 安装身份与状态绑定

原生安装必须拥有不可重复的 installation UUID。数据库、运行时目录、配置、Redis namespace 和
secret fingerprint 必须能证明属于同一 installation；仅凭相同库名或容器名称不够。

### F0

- installation UUID、lineage 和创建时间；
- append-only lifecycle operation event log：operation ID、类型、来源/目标 fingerprint、状态转换、步骤、
  checkpoint、checksum、操作者和时间；
- 可由 event log 重建的 operation head/projection；只允许以 CAS 单调推进，不得改写历史 event；
- 已完成 migration/legacy bridge/config migration 的步骤和 checksum；
- 当前 schema epoch、config epoch、data epoch；
- 已声明的 minimum reader/writer epoch；
- 已确认的来源 fingerprint；
- upgrade/restore 审计记录和备份引用。

### F1

- 数据库物理地址、库名和凭据；
- Redis 物理地址、DB number、namespace/prefix 和凭据；
- `/var/lib/v2board` 所在 PVC/bind mount；
- secret manager 中的 secret 版本。

这些项目可以迁移或轮换，但切换前后必须验证 installation identity，一旦发现 DB、runtime volume、
Redis 或 secret 不属于同一安装，启动和升级都必须 fail closed。

任何首次写入之前，必须先以同一 installation UUID 持久化状态为 `pending` 的 operation event；随后以
append-only event + CAS head 只能按 `running -> verifying -> completed` 前进，失败则追加
`failed`/`needs-recovery` 并保留最后 checkpoint。
原生 lineage 只能在 legacy bridge 全部验证后标记为可启动，但半迁移也必须能通过 pending operation
被准确识别，绝不能因崩溃落入“像旧库又像新库”的无主状态。

operation journal 的权威副本必须先于它保护的 DB/config/Redis mutation 存在；不能用“创建 journal 表”
这笔尚未记录的 DDL 来记录自己。fresh empty DB 应先在 durable runtime/external lifecycle store 以 fsync
方式写 pending event，再创建并镜像 DB installation ledger；两份记录通过 operation/installation UUID 和
checksum 对账。journal 不含明文 secret。

## 4. 数据库与持久化业务数据

### 4.1 通用 F0 规则

- 所有业务行的主键和自然键必须保持；不得重新编号或通过导出/导入生成新 ID。
- 每张使用自增主键的表必须保留安全的 next `AUTO_INCREMENT`，且严格大于现存 `MAX(id)`；不得在迁移
  后复用曾经分配的历史 ID。
- 所有关联必须保持同一父子关系；不得把孤儿自动挂到默认用户、默认套餐或默认组。
- 数据库中仍存在的 soft-deleted、已归档、已禁用、已取消和已完成记录仍是历史，不得因“不再显示”
  而丢弃。
- NULL、零、空字符串、空数组和字段缺失可能有不同含义；迁移不得统一折叠。
- `created_at`、`updated_at`、`paid_at`、首次/最后发现时间和统计时间不得统一改成迁移时间。
- JSON/text 中未知字段、未知 tag 和协议扩展按不透明业务内容保留；不得借迁移“清洗”。
- charset/collation 是数据语义。改变前必须检测大小写、重音、宽窄字符和截断碰撞。
- 数据库 session 语义保持：UTC、`utf8mb4_unicode_ci`、严格整数写入、
  `REPEATABLE READ`。业务日界线保持 UTC+8。
- 已发布 SQLx ledger 必须是正确 checksum 前缀；仅账本正确不等于真实 schema 正确，必须检测 drift。
- 迁移失败不得留下“DDL 已提交但步骤未记录”的不可识别状态。每一步必须可检测并可 resume；只有在
  writer fence 未解除、尚无新写入时才可明确恢复同一 operation 的一致快照，接流后按 13.4 节处理。
- current `0001_initial.sql` 中的 `CREATE TABLE IF NOT EXISTS` 不能作为旧库接管证明；同名旧表存在时
  必须先经过来源分类和具名 legacy bridge，不能被当成已创建的 native baseline。

### 4.2 身份、凭据与用户状态

以下全部为 F0，除非通过已有明确业务动作改变：

- `v2_user.id`、`email`、`password`、`password_algo`、`password_salt`；
- `token`、`uuid`；
- `invite_user_id`、`telegram_id`；
- `is_admin`、`is_staff`、`banned`；
- `group_id`、`plan_id`；
- `balance`、`commission_balance`、`commission_type`、`commission_rate`、`discount`；
- `u`、`d`、`transfer_enable`、`t`、`expired_at`；
- `device_limit`、`speed_limit`、`auto_renewal`、`remind_expire`、`remind_traffic`；
- `session_epoch`、`traffic_epoch` 及其他撤销/配额 epoch；
- `last_login_at`、`last_login_ip`、`remarks`、创建和更新时间。

旧密码摘要必须逐字节保留。系统可以在用户成功登录并验证明文后惰性升级为 Argon2；迁移器不得在
没有明文的情况下“转换”摘要，也不得强制全体用户重置密码。

密码三元组 `password/password_algo/password_salt` 只允许在成功校验明文后以 CAS 惰性升级，或由
显式改密/重置动作改变；真实改密、封禁和角色权限变化必须保持递增 `session_epoch` 的撤销结果。
`token + uuid` 只能由已授权用户的 reset-security 或管理员 reset-secret 业务动作一起轮换，升级本身
不得代行轮换。

`email` 的唯一性和比较依赖数据库 collation；不得先 lowercase 再去重，不得自动选择一个重复账号。

`session_epoch`、`traffic_epoch` 和 `credential_epoch` 只能保持或递增，绝不能回退，否则可能让已撤销
会话、旧流量报告或旧节点 token 重新有效。

### 4.3 订单、余额、佣金与统计

以下全部为 F0：

- `v2_order.id`、`trade_no`、`callback_no`，以及存在时的可信完整 callback digest；
- `user_id`、`invite_user_id`、`plan_id`、`coupon_id`、`payment_id`；
- `type`、`period`、`status`、`commission_status`；
- 所有金额字段、`paid_at`、创建和更新时间；
- `surplus_order_ids` 和订单之间的折抵关系；
- commission log 的邀请人、被邀请人、订单和金额；
- `v2_stat` 的全部历史行、record type/time 和聚合值；
- `v2_stat_user`、`v2_stat_server` 在既有 retention 窗口内的历史行，以及尚被 pending work/审计引用
  的行。

固定语义：

- 订单 `type=1/2/3/4/9` 分别为新购、续费、更换套餐、流量重置包、充值；
- 订单 `status=0/1/2/3/4` 分别为待支付、开通中、已取消、已完成、已折抵；
- `commission_status=0/1/2/3` 分别为待确认、发放中、有效、无效；
- stat `record_type='d'/'m'` 分别为日/月；未知历史值必须保留并报告，不得改成默认值；
- `period` 的公开标识包括 `month_price`、`quarter_price`、`half_year_price`、`year_price`、
  `two_year_price`、`three_year_price`、`onetime_price`、`reset_price`、`deposit`；
- `plan_id=0` 且 `period=deposit` 是充值订单，不是孤儿套餐关系；
- “每用户最多一个 status 0/1 订单”是业务约束，不是可删除的性能索引。

`trade_no` 必须逐值、逐字节保留且唯一，不得截断、重新格式化或重新生成。当前自动续费/佣金划转
存在生成值可能长于旧列宽的实现风险；成熟迁移前必须用扩列或生成规则修复并做边界测试，绝不能靠
数据库截断。`callback_no` 可作为显示标签；只有标签可证明完整，或收到已验签的原始 provider
transaction ID 时，才可生成完整 digest。legacy 行初始 `callback_no_hash=NULL` 是有效状态；不得从
可能已被旧 MySQL 截断的标签反推错误 digest。

用户 `commission_type` 的现行业务解释为：`0` 服从系统“首次”设置、`1` 每单有效、`2` 仅首次有效。
`actual_commission_balance` 和 commission log 是已发放事实，不能按当前配置重算；commission log
没有天然唯一键，不能仅按 `trade_no` 自动去重。

迁移前后至少按订单状态、支付方式和时间窗口核对行 ID 集合、金额合计、已付合计、余额使用、退款、
折抵和佣金合计；只比较总行数不够。

### 4.4 金额、流量、倍率与时间单位

单位及舍入规则均为 F0：

- 数据库存储的订单金额、余额、固定手续费、优惠金额、退款、佣金、礼品卡金额为整数“分”；
- 管理端显示或接收“元”时，API 边界执行明确的乘/除 100，不能把数据库单位改为元；
- `u`、`d`、`transfer_enable` 和流量统计为字节；套餐管理输入 GiB 时使用
  `1 GiB = 1_073_741_824 bytes`；
- `plan.transfer_enable` 的旧存储/管理语义为 GiB，`user.transfer_enable/u/d` 和 user/server stat 为
  字节；跨表示转换必须在明确边界执行，不能仅因同名就直接复制；
- coupon `type=1` 的 value 是分，`type=2` 是百分数点 `0..100`，不是 basis points；
- giftcard value 随 type 分别表示分、天、GiB、无值重置或套餐有效天数，不能统一当金额；
- server rate/traffic charge 使用十进制语义；旧版非数字/空 rate 的结果按 `0`，不能默认为 `1`；
- durable 用户配额落账分别计算 `round(raw_u*rate) + round(raw_d*rate)`，每项按现有
  half-away-from-zero 得到整数；traffic 页面/历史 stat 展示按其独立契约计算
  `(raw_u+raw_d)*rate`。二者不能合并成一个公式，也不得使用未经证明等价的二进制浮点；
- 时间戳字段为 Unix 秒；不得静默改成毫秒、数据库本地时间或浏览器本地时间；
- 数据库连接时区为 UTC，日历、统计和重置业务边界为 Asia/Shanghai；`record_at` 表示该业务日
  00:00 对应的 Unix 秒，不是 UTC 00:00；
- durable quota 与 display charge 分别使用上述公式，legacy traffic coercion 的业务结果不得漂移；
- 金额、汇率、百分比和佣金的现有舍入方向不得仅因库或语言变化而改变。

### 4.5 套餐、优惠券、礼品卡与邀请

以下为 F0：

- plan、server group、coupon、giftcard、invite code 的主键和 code；
- 套餐价格字段、流量、设备限制、速度限制、容量限制、renew/show/sort 和 group 关系；
- coupon 的 type/value、使用次数、用户次数、套餐和 period 限制、有效期；
- giftcard 的 type/value/plan、使用限制、有效期及每一条 redemption；
- invite code 的 owner、status、pv；
- 用户邀请关系、佣金归属和历史转账/提现金额。

coupon code、giftcard code、invite code 的唯一性是业务规则。发现重复时必须停止并交给操作者决定，
不得自动保留一条。

旧 `giftcard.used_user_ids` 到 normalized redemption 表的转换必须让每个**不同** user ID 对应一条
目标关系；重复按集合语义去重，非数组、非整数或不存在的 user 必须 fail closed。旧格式没有逐次
兑换时间，因此 unknown 必须保持可识别：目标 schema 应允许 unknown，或另存 explicit synthetic
provenance；可以用 deterministic fallback 便于排序，但不得把 giftcard created/updated time 宣称为
真实 redemption time。

固定 sentinel 和值域不得归一化：

- `user.expired_at=NULL` 与 `0` 不同；前者表示长期有效，后者保留旧版 falsy/过期或未配置语义；
- 套餐价格 `NULL` 表示不可购买，`0` 表示免费；旧自动续费对 NULL 的兼容结果也必须单独覆盖；
- `capacity_limit=NULL` 表示不限、`0` 表示无容量；`limit_use=NULL` 表示不限、`0` 表示已用尽；
- giftcard 的 `started_at/ended_at=0` 表示无起止限制，而 coupon `ended_at=0` 的既有结果是已过期；
- giftcard type `1/2/3/4/5` 与 coupon type `1/2` 不得重编号；giftcard type 5 的 value=0 保留长期
  有效语义；
- invite code `status=0/1` 为可用/已消费；`invite_never_expire` 下使用后仍可保持 0，`pv` 是累计访问
  事实，不能按注册数重建；
- `plan.reset_traffic_method=NULL` 表示继承全局；`0/1/2/3/4` 分别对应每月 1 日、到期日、不重置、
  每年 1 月 1 日、到期周年日；旧版“全局 3 + plan NULL”的 3→4 fall-through 计费结果必须保留；
- `ticket_message.user_id=0` 是允许的管理员作者；reconciliation `order_status=-1` 是验签付款找不到订单；
- parent/plan/group、金额、callback 和 paid time 的 NULL 不得擅自改成 0 或空串。

### 4.6 支付方式、回调与 reconciliation

以下为 F0：

- `v2_payment.id`、`payment` provider code、`uuid`；
- 每个支付版本的完整 config、notify domain、手续费、启用和归档状态；
- 已归档支付版本的回调路由和验签材料；
- order 与 payment version 的绑定；
- callback identity、payment reconciliation 行、原始摘要、处理状态和审计信息。

支付配置是**不可变验签版本**。轮换 provider、密钥或验签配置必须归档旧行并创建新行；不得原地覆盖
会影响历史回调的配置。已禁用或已归档支付方式不得用于新 checkout，但必须继续验证迟到回调。

支付 provider code 和 config key 是持久化协议，不得重命名后只迁当前启用项。
config JSON 的类型、未知字段和 secret 字段必须逐语义保留，不能经过管理表单往返后丢失。
旧 payment UUID 合法长度为 8 字符，native 新值可为 32 字符；旧值不得因“不满 32 位”被拒绝、补齐或
重生成。reconciliation 的 `resolved_at=NULL` 表示未处理，`settled_amount=NULL` 表示 provider callback
没有可用的已认证金额，不得转换为 0。

### 4.7 节点、路由和节点凭据

以下为 F0：

- server group、server route 和各协议节点的 ID；
- node type、group membership、route、parent、host、port、server port、rate、show、sort；
- 协议专属字段和 JSON 配置中的公开键名及含义；
- `v2_server_credential` 的 node type、node ID、credential epoch；
- 节点统计和用户流量归属。

节点持久身份是 `(node_type, node_id)`，不是全局 `node_id`；不同协议表中相同数字 ID 是不同节点，
credential、stat 和 report 迁移都必须用二元 identity。

节点 `group_id` 的 current native 语义为非空 JSON 数组，成员必须引用真实 group。旧列虽以 text/varchar
存储，业务值仍必须是合法 JSON array；只允许把“文本编码的合法 JSON 数组”转换为 native JSON 表示，
成员集合不得改变。scalar `"1"`/`1`、空数组或 malformed 值必须归 drift/fail closed，除非具名 profile
另有经过证明的映射。

节点 credential epoch 只能保持或增加。节点 ID 或 node type 改变会改变派生 token，普通升级不得做。

### 4.8 工单、内容、邮件和日志

以下为 F0：

- ticket 和 ticket message 的 ID、owner、author、subject、message、level、status、reply status 和时间；
- `ticket.status=0/1` 为开启/关闭；`reply_status=0/1` 分别表示最后由用户回复/待运营回复、最后由运营
  回复。管理员回复可能重新打开已关闭 ticket；
- ticket level 的现有值域仅 `0/1/2`，不得重编号或用显示文案代替；
- `ticket_message.user_id=0` 可以表示管理员回复，不得当成孤儿 user；
- 每用户最多一个开启工单是业务约束；
- knowledge、notice 的 ID、语言、分类、正文、可见性、标签、排序和时间；
- commission log 的全部发放历史；
- mail/application log 在各自既有 retention policy 到期前的记录；
- 已持久化 mail outbox batch、recipient、message identity、payload hash、lease/retry/terminal 状态。

日志和终态 outbox 可以由独立 retention policy 删除，但应用升级本身不得提前删除或重置保留窗口。

### 4.9 幂等与异步处理状态

以下在其有效保留窗口内均为 F0：

- traffic report idempotency key、payload hash、epoch、items 和 applied state；
- mail outbox batch key、message ID、payload hash、recipient 和 terminal state；
- payment callback/reconciliation identity 和 resolution；
- 已接受但尚未完成的订单、佣金、续费、流量结算、工单和提醒工作；
- cleanup retention 的既有起点和“不得删除 pending/leased work”语义。

重复请求使用相同 key 但不同 payload 必须继续拒绝。升级不得清空 ledger 后让旧 key 重新可用。

旧 Laravel queue/Horizon payload 不得交给 Rust worker 直接执行。迁移前必须完成以下一种有审计的处置：

- 停止新入队并排空可安全执行的旧任务；
- 将业务结果转换成受支持的 native outbox/ledger；
- 对无法转换的任务生成明确清单并由操作者处理。

不得让旧 scheduler/Horizon 与 Rust worker 同时写业务数据。

### 4.10 唯一性、关系、幂等终态与保留期

以下唯一性及其业务结果是 F0；发现旧数据违反时必须停止，不得自动 dedupe、改码或取消：

- user 的 email、token；order 的 `trade_no`，以及每用户最多一个 status 0/1 order；
- 每用户最多一个 status 0 ticket；coupon/giftcard/invite 各自的 code；
- payment 的 `(payment, uuid)`，不是 uuid 全局唯一；
- giftcard redemption 的 `(giftcard_id, user_id)`；
- reconciliation 的 `(payment_id, callback_no_hash)`；
- traffic report key、`(report_key,user_id)`；mail batch key、`(batch_key,recipient)` 和全局 message ID；
- `v2_stat` 的 `record_at`、server stat 的 `(server_id,server_type,record_at)`、user stat 的
  `(server_rate,user_id,record_at)`；这些 key 不因 `record_type` 看似遗漏而擅自修改。

目标外键只按 current schema 的受控集合建立：plan→group、user→plan/group、order→user、非充值
order→plan、giftcard→plan、invite code→user、ticket→user、message→ticket、redemption→giftcard/user、
traffic item→report/user、reconciliation→payment。不得给 `ticket_message.user_id` 自动补 user FK，也
不得自动删除或重新绑定旧订单中的历史 coupon/payment/invite dangling reference。

异步终态的表示同样不可丢：

- mail item 已删除而 batch tombstone 仍在可以表示已经成功；不得把 tombstone 当空垃圾后重复发信；
- explicit traffic report 已 applied 后需在幂等窗口保留 tombstone；`i-` implicit report 仅在原子应用后
  才可删除；pending header 和全部 items 必须一起存在；
- traffic report 的 epoch 与 user `traffic_epoch` 不同表示旧周期报告，只能按既有规则丢弃，不能计入
  新周期；
- reconciliation 的 first/last seen、occurrence count、reason、digest、金额、actor/note 和 resolution
  是支付审计事实；resolved 不能由升级自动回退或改写。

retention 到期前的数据是 F0；到期后也只能由独立 cleanup policy 清理，而不是由升级器顺带删除。
当前最低语义包括：`v2_log` 约 1 个月、user/server stat 约 2 个月、mail terminal/batch tombstone 与
explicit traffic tombstone 默认 90 天。`v2_stat` 没有可假定的 retention，永久按 F0。旧
`failed_jobs` 必须先停 producer 并排空、转换或导出；历史残留表必须先 quarantine、记录 checksum 并
证明无 reader，不能按表名猜测后删除。

### 4.11 Legacy 到 native 的确定初始化

reference 不存在的 native 字段不得交给随版本变化的默认值。唯一旧版 bridge 固定初始化：

- 现存 user 的 `session_epoch=0`、`traffic_epoch=0`；
- 每个现存 `(node_type,node_id)` 的 `credential_epoch=0`；
- 旧 payment 的 `archived_at=NULL`；旧 order 的 `callback_no_hash=NULL`；
- 其余新增字段必须逐项写入 compatibility profile、来源值和初始化证明，未分类仍为 F0/停止。

`scheduled_traffic_reset_key` 不能无条件置 NULL 后立即启动 reset worker。bridge 必须审计切换业务日内
旧 reset 是否已执行；能证明已执行时写入对应 `YYYY-MM-DD`，不能证明时暂停 native reset 到下一个
安全业务日并记录，绝不能让同一天重复清零。

旧版用户的永久订阅凭据是 MySQL `v2_user.token`；它是 F0，必须与用户 ID/`uuid` 保持原值迁移。
方式 0 的订阅 URL 直接携带该 token。Redis 中的 `otp_{permanent}` / `otpn_{temporary}` 只是方式 1
的 24 小时临时映射；方式 2 的 URL 由用户 ID、MySQL 永久 token 与时间窗计算，签发时可以
完全不写 Redis，首次验证时才可能写入到本窗口结束的 `totp_{temporary}` cache。因此临时
OTP/TOTP key 不能替代 MySQL 永久 token 迁移，而 Redis 扫描为零也不能单独证明已签发 TOTP URL
全部过期；还必须有 machine fence 后的最长有效窗口 proof。

旧 Redis hash `v2board_upload_traffic`、`v2board_download_traffic` 是已经接受但尚未落入 user `u/d`
的权威流量增量，不是已落库历史流量报表的缓存。旧版每次上报还可能分别产生 TrafficFetch、
StatUser、StatServer 三类任务。
legacy bridge 必须停 producer，取得一致 inventory，安全排空/转换三类任务和两个 hash，核对 user 与
server/user stat，再证明它们为空后初始化 traffic epoch 并切流。旧 `traffic:update` 存在先删 Redis
再写 DB 的窗口，不能直接作为迁移 drain 工具；必须用不会在崩溃时丢流量的 durable bridge。

## 5. 认证、会话与浏览器状态

### 5.1 F0

- 用户端和管理端共享 `localStorage["authorization"]`；
- HTTP API 使用原始 authorization 值，不得给普通用户 API 强制增加 `Bearer ` 前缀；
- 403 的既有安全结果：客户端清理凭据和 session-scoped query cache，并跳转登录；
- password reset、ban、角色变化等安全动作通过 epoch 撤销旧会话；
- `localStorage["umi_locale"]`、`window.g_lang`、`window.g_langSeparator` 和 `i18n` cookie；
- API locale header、支持的 locale code 及语言持久化结果；
- `/api/v1/passport/auth/token2Login` 的 `token` 输入分支及 302 redirect 语义；
- 由实际 quick-login API producer 签发的 `/#/login?verify=...&redirect=...` URL、一次性消费与站内
  redirect 语义；
- redirect 必须保持站内路径约束，不能在升级中放宽成 open redirect。

### 5.2 F1

- `APP_KEY` 原始字节，包括旧版值可能包含的 `base64:` 前缀；
- auth/session Redis 物理位置与 namespace；
- session TTL、privileged TTL、step-up policy 的有效值。

`APP_KEY` 参与旧 JWT 校验、默认后台路径和 Telegram webhook secret。lifecycle spec 必须手工填写明确
目标值；若不轮换就手工复制原值，若轮换则必须同时填写显式 secure path、保持全量登出，并确认
Telegram 等受影响集成已关闭或重注册。升级器不得自行生成或猜测。

当前实现只接受单一 `APP_KEY`，尚无 keyring/dual verifier；因此不允许无停机轮换。唯一旧版的
维护窗口迁移只有在 lifecycle spec 明确选择全量登出、secure path 和第三方处置后才能使用新值。

### 5.3 F2

- 旧 JWT 的读取；
- `auth_data` query/body 参数兼容；
- 旧 Laravel Redis session key/value 格式；
- md5、sha256、md5salt、bcrypt `$2y$` 等旧 password verifier；
- quick-login `TEMP_TOKEN` 的 60 秒 TTL 和一次消费；
- 注册/忘记密码共用的 `EMAIL_VERIFY_CODE` 的 300 秒 TTL，以及 60 秒发送限流；
- 一次性/TOTP 订阅 token、`otp_`/`otpn_`/`totp_` Redis 映射及其原 TTL/消费语义。

旧 password verifier 只有在数据库 inventory 证明对应 hash/algo 为零、回滚窗口结束且不再允许导入
旧来源后才能退役；不能仅以“新密码都用 Argon2”为依据删除。

本安装的唯一旧版迁移固定选择**全量登出**，不实现 Laravel session converter/dual-reader：

- spec 强制 `legacy_auth_params_enable=false`、`legacy_jwt_cutoff_unix=0`；
- source 与 target Redis 必须是不同 DB/namespace，target 必须为空；不复制旧 session 或 native session；
- 旧普通/admin JWT 在切流后都必须 403，前端清 `authorization` 并跳登录；
- 固定 `legacy_cache=discard_ephemeral_after_fence`：旧 API/worker/node producer 全部停止后，旧登录态、
  限流、节点在线状态和可重建统计 cache 不复制；这项决定不涵盖流量、队列或未知 durable key；
- 绝不 `FLUSHDB`：旧 Redis 的未落库流量、queue、OTP/TOTP 订阅 token 必须先分别 inventory；流量/queue
  非零时阻断，已签发订阅 token 非零时也阻断并要求先自然到期或另行决定；
- password hash 仍原值迁移，用户重新登录时按既有惰性规则升级 verifier。

改变 app domain/origin 时，浏览器 localStorage 不会跨 origin。轮换必须提供受控、一次性的 signed
handoff，或明确记录需要重新登录；仅证明服务端 session 仍在不能宣称浏览器会话连续。

## 6. HTTP API、公开 URL 与前端路由

### 6.1 API F0 契约

所有真实客户端、节点或第三方消费的 API 都按 F0 处理，包括：

- `/api/v1`、已有 `/api/v2` 路径和动态 admin/staff 路径；
- HTTP method；即使历史 mutation 使用 GET，也不得在原路由上直接改成 POST；
- path、query、header、form body 中参数所在位置；
- 参数名、大小写、空值语义、数组 `field[0]` form encoding；
- 普通 API 的原始 authorization header；
- `Content-Language`、`Idempotency-Key`、`x-v2board-step-up` 等公开 header；
- JSON、form-urlencoded、msgpack 等 content type/response format；
- `{data}`、`{data,total}`、`{message}`、422 `{message,errors}`，以及 endpoint-specific 非普通
  envelope；
- endpoint-specific status code 及其安全结果，不得把 401/403/422 当成可互换；现有前端只对 403
  执行清 auth 和跳转时，该差异就是契约；
- API 字段中的金额、流量、时间、枚举和 NULL 语义；
- 外部可见的排序、分页 raw page、ID passthrough 和幂等结果。

可以添加向后兼容字段；不得在 v1 原地删除、改名、改类型或改变单位。确需破坏时新增版本或使用
双读/双写，并在 F2 窗口后退役旧版本。

route audit 和 interaction parity 是契约证据，不是契约来源。真实外部消费者和 Rust backend 语义优先。

`token2Login` 的旧分支必须有 fixture：`?token=` 返回 302 与 `Location`；`?verify=` 一次消费后返回 auth
JSON；两者都缺失时保留旧版空 200，直到明确版本化。节点 API 还必须覆盖 uniproxy `{data:true}`、
tidalab `{ret:1,msg:"ok"}`，以及部分 v2 endpoint 用 HTTP 200 返回
`{status:"fail",message}`/`msg` 的结果，不能统一包成普通 envelope。

### 6.2 公开 URL F0 契约

以下 URL/参数可能存在于邮件、聊天记录、书签、客户端和支付平台中，必须长期可解析：

- `/#/login?verify=...&redirect=...`；
- `/#/register?code=...`；
- `/#/forgetpassword`；
- 旧版邮件中的 `/#/subscribe`，至少必须稳定落到已认证用户的订阅/dashboard 能力；
- `/#/order/{trade_no}`；
- `/#/ticket/{ticket_id}`；
- `/#/knowledge`、`/#/knowledge?id=...` 及当前知识库 `id` opening；
- 用户端现有 Hash route 集合；
- 当前动态后台路径在其 F1 轮换/回滚窗口内，以及该路径下的 admin Hash route 集合；
- 支付 callback、Telegram callback 和订阅 URL。

新路由可以替代展示，但邮件/客户端已发布的 F0 路由必须作为 alias 继续解析并保持安全结果；仅修改
新版前端链接不够。动态 admin 外层 `secure_path` 是安全敏感 F1，可在明确窗口、所有入口更新和审计后
退役旧值；其内层 Hash route 与 auth/redirect 结果仍是 F0。

当前用户 Hash route 清单为：`/`、`/login`、`/register`、`/forgetpassword`、`/dashboard`、
`/plan`、`/plan/:plan_id`、`/order`、`/order/:trade_no`、`/profile`、`/invite`、`/ticket`、
`/ticket/:ticket_id`、`/knowledge`、`/node`、`/traffic`。

当前 admin Hash route 清单为：`/`、`/login`、`/dashboard`、`/config/payment`、
`/config/system`、`/coupon`、`/giftcard`、`/knowledge`、`/notice`、`/order`、`/plan`、
`/queue`、`/server/group`、`/server/manage`、`/server/route`、`/ticket`、
`/ticket/:ticket_id`、`/user`。它们位于动态 admin path 下；动态外层 path 属于 F1，内部 Hash route
及其 auth/redirect 结果属于 F0。

知识正文中的 `copy()`/`jump()` hook 是 backend 内容契约。迁移或 renderer 改造不得让既有正文失去
这些动作，也不得因 refetch 优化缓存 backend 的非幂等替换结果。

### 6.3 应用级 Tier-1 结果清单

本节把第 6.1 节的通用规则落实到现有 surface。列表中的 endpoint、payload、单位、ID、路由和安全
结果均为 F0；新增版本可以改变纯展示，但不得让这些结果失去自动化覆盖。

- Auth：register/login/forget/token2Login payload，`verify`/invite code，reCAPTCHA，email verification，
  auth redirect safety，language/auth persistence。
- User shell/dashboard：未登录跳转、订阅 URL/QR/import link、notice `弹窗` 自动弹出条件、reset-package
  order、new-period mutation。
- Commerce：plan filtering、`capacity_limit` sold-out、coupon check（包括空 coupon）、save-order、
  unfinished order、cancel `{trade_no}`、change-subscription、payment method、StripeCredit
  PaymentIntent/Element、QR/redirect checkout、支付返回路由。
- Profile：auto-renewal、email reminder、password-change 后安全跳转、giftcard redeem、deposit order、
  Telegram bind/unbind、`/user/resetSecurity` token/UUID rotation。
- Node/traffic：subscribe-first fetch、empty-state subscribe/renew 路由、`is_online`/`server_rate`、
  `(u+d)*server_rate`、legacy charge coercion。
- Invite：copy-link URL、`/user/invite/save`、`100*amount` transfer payload、withdraw method/account、
  commission cents `amount/100`。
- Ticket：ticket ID passthrough、reply/create/close payload 和 detail route。
- Knowledge：fetch locale、URL `id`、detail fetch、正文 `copy()`/`jump()`、每次打开/refetch 当前文章。
- Admin：所有 config/coupon/giftcard/notice/knowledge/plan/server/user/order/ticket create/edit/delete
  endpoint 和 body；`limit_plan_ids[0]` 等 form array；金额分转换；list/filter/pagination query；
  admin auth/session 和动态 path route。

本节未列出的 spinner、toast、modal、poll/debounce、refetch timing、表格滚动和纯显示格式默认不提升为
F0；若其结果映射到数据、外部 URL、安全或 payload，仍按“未分类即 F0”。TOS 配置启用时的前端提交
gate 和 dashboard alert routing 继续保留行为覆盖，但属于可由 surface owner 调整的 Tier-2 UX；其 API
payload、外部 URL 和 auth 安全结果仍是 F0。

## 7. 订阅凭据与协议输出

### F0

- `v2_user.token` 和 `uuid` 原值；
- 已签发 `{subscribe_path}?token={token}`、OTP/TOTP 或其他完整订阅 URL 的形状、token 参数名和可达性；
- `show_subscribe_method=0/1/2`、`show_subscribe_expire` 对已签发 URL 的解析和失效结果；
- `flag`、User-Agent 探测、Host 使用和协议选择；
- subscription response headers、文件名/更新信息等客户端消费字段；
- General/Base64 URI、Clash、Stash、sing-box current/legacy、Surge、Surfboard、Loon、Quantumult X、
  V2RayTun、Shadowsocks SIP008、Shadowrocket、SagerNet 等实际支持分支的外部语义；
- Hiddify、Sing-box、Shadowrocket、Quantumult X、Surge、Stash、ClashX、ClashMeta、NekoBox、
  Surfboard 等一键导入 URI 被实际输出时的 scheme、encoding、title 参数结构和 `flag`；
- `/api/v1/client/app/getConfig`、`/api/v1/client/app/getVersion` 等已安装客户端 API；
- 节点 `is_online`、rate、group、route、协议 JSON 字段到订阅输出的解释；
- 已授权用户 `resetSecurity` 或管理员 reset-secret 才能轮换 token/UUID 的业务边界。

### F1

- `app_url`、`app_name`（影响未来 import title）、`subscribe_url`、`subscribe_path`、
  `show_subscribe_method`、`show_subscribe_expire`、域名和 TLS endpoint。

改变这些值必须保留旧 path/domain 的服务或代理 alias，直到有真实客户端采用率证明可以退役。订阅
客户端不一定正确跟随 HTTP redirect，因此旧 path 应直接提供内容，不能只返回 301。

旧 `subscribe_url` 可以是逗号分隔的多 host 集合并随机签发。迁移 inventory 中出现过的所有
host/path 都必须直接提供订阅，或由操作者证明从未发布后才能退役；只取第一项会破坏已签发 URL。

### F2 token/mode 兼容

改变 `show_subscribe_method` 不能立即废掉旧 URL。mode 1 的 `otp_`/`otpn_` 双向 mapping、86400 秒 TTL
和一次消费，mode 2 的 `totp_` cache、`show_subscribe_expire*60` timestep/TTL、HMAC 输入与时间窗口，
以及 mode 0 直传 token 都必须双接受到明确截止点；或在切换前主动更新全部真实消费者并取得 drain
证据。配置的未来生成方式是 F1，已经发给用户/客户端的完整 URL 在 reset-security 或有消费者证明前
是 F0。

只有“停止签发”而旧 verifier 仍可能服务时，mode 2 token 可在 bucket 尾部首次验证并再缓存一个完整
timestep，因此保守窗口至少为 `2*timestep + clock_skew`。只有 machine fence 同时证明旧 verifier 已停，
才可收紧为一个已验证 timestep 加裕量。

某个 import 菜单针对什么 UA/platform 显示属于 Tier 2；一旦实际输出，URI 结构/编码/flag 是 F0。
`title` 参数的结构和 encoding 是 F0，其文本值跟随可显式修改的 F1 app name，不冻结成固定字符串。

### 旧版自定义规则

旧版 `resources/rules/custom.*` 是 operator state，不得静默忽略：

- `custom.clash.yaml`、`custom.stash.yaml`、`custom.app.clash.yaml`；
- `custom.sing-box.json`、`custom.sing-box.old.json`；
- `custom.surge.conf`、`custom.surfboard.conf`；
- 迁移检查发现的其他 `custom.*` 文件。

每个文件必须由严格 parser 读取，转换到受支持的 native 表示，原子安装到 runtime，并通过对应格式
的输出验证。若 native 尚不支持某种 override，迁移必须阻止切流并列出文件；不得盲目复制、执行旧
模板，或自动改用 embedded default。

## 8. 支付、结算和外部 Webhook

### 8.1 永久 F0

- callback path `/api/v1/guest/payment/notify/{method}/{uuid}`，其中 `method` 的值就是持久 provider code，
  与 payment row/UUID 的绑定不得混淆；
- 已存在 callback 的 GET/POST method 接受范围；
- provider code、payment UUID、历史 config 和签名算法；
- outbound checkout request 的 endpoint、字段、callback/return URL、金额/币种转换和 provider metadata；
- checkout response envelope 的 type `-1/0/1/2`、data shape，以及其表示的既有 QR/redirect/其他
  checkout 结果；
- `trade_no`、user/order/payment binding、金额和币种；
- provider transaction ID、callback digest、重放检测和幂等结果；
- provider 要求的精确响应 body bytes、HTTP status 和 `Content-Type`；
- 已发给 Stripe 等外部系统的 metadata 字段和含义；
- signed webhook 的签名输入（包括 exact raw bytes）、命名 header、算法和允许时间边界；
- 已成功支付只开通一次、未知订单不自动认领、金额或币种不匹配不结算。

安全加强不得通过接受更少证据来换取兼容。metadata 不完整、签名无法验证或金额无法绑定的回调只能
进入 reconciliation，不能自动开通订单。

当前 provider code 清单为 `AlipayF2F`、`BEasyPaymentUSDT`、`BTCPay`、`CoinPayments`、
`Coinbase`、`EPay`、`MGate`、`StripeALL`、`StripeAlipay`、`StripeCheckout`、
`StripeCredit`、`StripeWepay`、`WechatPayNative`。每个 provider 的 config key、signature/header、
raw-body 规则和精确 ACK（例如 `success`、`ok`、`IPN OK` 或 XML）都属于同一 F0 contract。
签名 header 包括 provider 实际使用的 `stripe-signature`、`hmac`、`x-cc-webhook-signature`、
`btcpay-sig` 等；必须从实现 fixture 生成逐 provider 清单，不能只测“有签名头”。等价的内部验签
控制流顺序可以重构，冻结的是输入、接受/拒绝、审计和结算结果。

`StripeCredit` 的独立 `/user/order/stripe/intent` 流程还必须冻结 PaymentIntent prepare payload、
Payment Element confirm 语义，以及绝对 `window.location.href` return URL 中的
`/#/order/{trade_no}` Hash path；它不是 checkout envelope type。已经创建的外部 object 所带
metadata/callback/return URL 是 F0，未来 provider config 或 secret 的变更走新 payment version 的 F1
轮换。

### 8.2 唯一旧版的 Stripe F2 过渡

本安装声明从未配置 Stripe，采用 `assert_none` 而不是实现旧 Stripe bridge。inspect 必须自动查询所有
`Stripe*` payment row、绑定它们的 status 0/1 order，以及存在时的 callback/reconciliation inventory；
全部为零才标记 `not-applicable`。任一非零即 fail closed 并恢复使用本节下述通用处置，不能只信口头
声明。

当前 bounded preflight 只实现前两类数据库计数，尚未证明 provider 侧 callback/reconciliation
inventory；这项差距因此进入 `implementation_blockers`，当前不会输出可操作的 `compatible` 或
`ready_for_confirmation`。完整实现不得通过降低本节要求来消除这项差距。

旧版创建的 Stripe 对象缺少 current verifier 要求的部分 metadata；旧 Checkout 可能只有
`client_reference_id`。切换前必须先建立 pre-cutover Stripe inventory，并执行以下一种策略：

1. 暂停新订单，排空、取消或安全完成所有旧 Stripe session/intent，切换后重建；或
2. 只将签名正确但 metadata 不足的旧事件写入具名 legacy reconciliation，由人工或额外 provider
   查询验证；不得直接自动开通。

旧对象与新对象必须通过创建时间、cutover ID 或持久化 origin 明确区分；不得让宽松 verifier 处理
切换后创建的新对象。

## 9. 节点控制面

### F0

- `/api/v1/server/{class}/{action}`、`/api/v2/server/config` 等节点路由；
- node class/type aliases（包括 `v2ray -> vmess`、`hysteria2 -> hysteria`）、`node_id`、`node_type`；
- token 可出现的位置及冲突 token 必须拒绝的规则；
- JSON/msgpack、`x-response-format`、ETag/304；
- node config/users/traffic payload 字段、类型和缺省语义；
- `user_id/u/d`、`Idempotency-Key`、`report_id`、`idempotency_key`；
- 相同 report key + 相同 payload 的幂等结果和不同 payload 的拒绝结果；
- uniproxy `{data:true}`、tidalab `{ret:1,msg:"ok"}` 和 v2 HTTP 200 fail envelope；
- 流量先持久化后应答；对同一稳定 report identity，worker 只应用一次的业务结果。

### F1

- `server_token` master key；
- 每节点 scoped token 和 credential epoch。

`server_token` 是所有 `n1_...` 凭据的派生主密钥。普通迁移必须原值导入。轮换 master key 需要
独立的双 key/逐节点更新协议；仅增加某节点 epoch 的正常 rotate 操作不得影响其他节点。

当前 verifier 只支持一个 master key；在 keyring/key-id 与双 master 验证实现前，master rotation 是
明确 blocker，只能保留原值，不能假装逐节点更新已经提供 master 回滚能力。

### 唯一旧版的维护切换

本安装不实现在线 global-token 双协议窗口，固定选择维护切换：

1. inventory 所有节点；零节点时自动跳过；
2. 停止旧 API/Horizon/scheduler 和每台 node reporter，禁止新上报；
3. 排空 queue，核对并结清两个旧 Redis traffic hash，证明 DB/stat 一致；
4. source 最终只读快照克隆到独立 MySQL 8.4 target，在 target 上完成转换；
5. 保持 `server_legacy_token_enable=false`、`server_require_idempotency_key=true`；
6. 为每个节点生成 scoped token，手工更新 endpoint/token 和支持稳定 idempotency key 的 reporter；
7. 每节点验证 config/users 拉取和一批 traffic 只计费一次，全部通过后才恢复上报。

迁移期间旧/新 reporter 不得双跑。这里的“节点掉线”指面板鉴权、配置同步和流量上报被拒，不是节点
表数据丢失；旧 agent 共用 global token 且无稳定 report ID，而新版严格默认会同时拒绝这两种形状。

## 10. Telegram、邮件和浏览器第三方集成

### 10.1 Telegram

F0：

- `telegram_id` 绑定关系；
- `/bind <subscribe_url>`、`/unbind`、`/traffic`、`/getlatesturl` 等既有命令名、授权、数据和业务动作；
- `chat_join_request` 根据绑定用户和订阅有效性执行 approve/decline 的安全结果；
- webhook update 的幂等消费；
- bot token 对应的 bot identity。

命令精确文案、表情和换行不是 F0；不得改变的是命令入口、身份绑定、授权、数据含义和动作结果。

F1：bot token、APP_KEY、app URL 和 webhook secret。任一变化都必须重新注册 webhook 并验证
`getWebhookInfo`。

唯一旧版使用 query `access_token=md5(bot token)`，native 使用 Telegram secret header。旧版切换
必须主动执行 `setWebhook` 并验证成功；复制 bot token 但不更新 Telegram 端不算迁移完成。当前
native handler 不接受旧 query token，因此实施时必须二选一：先实现有界 query-token adapter；或在
维护窗口排空/记录 pending updates 后原子 `setWebhook` 切换并验证。旧 query token 兼容属于 F2，
只有新 webhook 已确认且 pending update 有 durable 处置后才退役。

### 10.2 邮件

F0：

- 已发送邮件中的站点根 URL，以及 classic 模板实际发出的 `/#/subscribe`、`/#/knowledge`；
- 注册/忘记密码邮件中的 `EMAIL_VERIFY_CODE`、5 分钟有效语义和业务动作；
- pending outbox 已冻结的 exact sender、recipient、payload、message ID 和去重结果；
- reminder/notify 邮件链接的既有可达性。quick-login URL 属于实际 API producer 契约，不因未证明可达的
  `mailLogin` 模板而成为邮件契约。

SMTP host/port/username/password/encryption/from address、邮件 template selection 为 F1 operator state，
普通升级必须保留有效值。SMTP host/credential/port/encryption 是 transport，显式轮换后可以并通常
必须投递既有 pending mail；pending sender/from、recipient、subject/body/envelope/message identity
不能随 transport 轮换改写。template/from 内容变更只影响未来 enqueue。模板视觉、翻译和日期显示
格式可以改变，但不能改变 code、URL、金额、时间点/时区或业务动作含义。模板文件只有在调用链证明
存在实际 producer 后才成为外部契约证据，不能仅因文件名存在就推断功能。

### 10.3 Crisp、Tawk 及类似浏览器集成

F0：

- Tawk visitor identity，现有 `name=email`、`email=email` field binding；
- Crisp `user:email` 和其 SDK 要求的 nested command-array payload shape；
- `Balance`、`Plan`、`ExpireTime`、`UsedTraffic`、`AllTraffic` 等 session data key、单位和用户绑定；
- 只有成功取得当前已认证用户 info/subscribe 数据时才能上报；fetch 失败不得发送旧用户 payload；
- refetch 后的上报仍必须绑定当前用户，登出时清理集成状态，不得泄漏上一用户；
- operator 的受支持 custom HTML 若负责 bootstrap widget，迁移后必须显式验证仍能启动，或阻止切流。

未来新增或更换客服系统时，旧 payload 不能在外部仍消费期间原地改名或换单位。
具体 refetch 次数/时点属于 Tier 2；每一次实际发送的 identity、shape、单位和用户隔离属于 F0。

## 11. 配置、密钥与 native runtime 文件

### 11.1 有效配置保值规则

唯一旧版迁移固定使用 `manual_only`：不得从旧 `.env`、`config/v2board.php`、theme config 或 operator
文件推断和导入目标值，更不得 `eval`、include 或启动 PHP。工具最终必须只对这些旧 artifact 做存在性、
类型和 checksum inventory；目标 native 配置由操作者在版本化 spec 中完整手填，未知 key、缺失 key、
placeholder 或未确认 artifact 一律阻断。若操作者要保留某项行为，必须在 spec 中填写对应有效值；
若明确改变或放弃，则按本文 F1/F2/F3 的审批和 proof 要求记录，不能由默认值静默决定。

当前 v2 另要求操作者手填旧 cache driver、Redis prefix、订阅 method/expire 和停止签发时间，供 bounded
preflight 定位来源数据；这些是尚未机器验证的 source declaration，不属于已证明的目标配置。未来 apply
必须用不执行 PHP 的安全解析器或等价机器证据核对，不能因 `manual_only` 而永久信任人工声明。

这项 future source proof 固定如下，不能退化成继续相信表单：

- 证据是 fence 后只读旧根目录内的 `bootstrap/cache/config.php`，spec 记录固定相对路径和 SHA-256；
  缺失、symlink、摘要变化或无法验证 regular file 一律 `unsupported/unverifiable`；
- parser 只接受 Laravel `var_export` 生成的 scalar/null/nested-array grammar，禁止执行 PHP、`eval`、
  include、函数、常量求值、对象或尾随代码；
- 只提取并证明 `cache.default/prefix`、两个 effective Redis identity/prefix、queue driver/connection/name、
  `v2board.show_subscribe_method/expire`，并按 pinned Laravel URL/component precedence 与 spec 连接比较；
- cache 或 queue 不是当前支持的 Redis 形状、连接 identity 不一致、method/expire 超出范围或字段无法唯一
  解释时立即阻断；解析出的密码只在内存比较，报告不得输出；
- 临时订阅链接的停止签发时间不得由操作者回填，必须来自 future append-only operation journal 记录的
  machine fence checkpoint；method 1 从该时刻至少等待 24 小时，method 2 等待已验证 timestep 加时钟裕量；
- config SHA、datastore identity 或上述 facts 在 validate/plan/fence 间变化时废弃旧 plan 并重新检查；
  解析值只用于 source gate，绝不 merge/copy/default-propagate 到 target runtime。

对于既有安装，以下所有**有效值**均为 F0（值本身可能属于 F1）：

- 站点：app name/description/URL、logo、TOS、currency/symbol、force HTTPS；
- 注册与安全：stop register、invite force、email verify/whitelist/Gmail restriction、reCAPTCHA、
  registration/password limits、safe mode、secure path；
- 邮件：template、host、port、username、password、encryption、from address；
- Telegram：enable、bot token、discussion link；
- 邀请和佣金：各层比例、首次佣金、自动确认、提现限制/方法/开关、invite generation limit；
- 订阅与订单：subscribe URL/path、试用套餐/时长、换套餐、剩余价值、流量重置方式、新周期、事件开关、
  deposit bonus、信息节点显示；
- 节点：server API URL/token、push/pull interval、上报阈值、device limit mode、日志和 legacy/idempotency
  rollout flags；
- 前端和下载：theme color、background、custom HTML、各平台 version/download URL；
- runtime security/operations：CORS origins、trusted proxies、timeouts、session/step-up TTL、pool limits、
  worker heartbeat/shutdown/cleanup/retention 配置；
- 数据存储和路径：database URL、Redis URL、runtime/config/rules/frontend path；
- 所有未来新增且在升级前已经显式设置的配置。

操作者必须先人工核对旧 installation 的 effective value 及其 environment/runtime/default provenance，再把
最终目标值显式写入 spec；迁移器不替操作者重放旧优先级。生成的 native 文件固定
`configuration_source=file_only`，文件中显式值（包括 `null` 和空数组）不得再被业务配置环境变量覆盖。
database/Redis 的 installation binding 和逻辑 identity 是 F0；其物理 endpoint、库名、凭据和
frontend/runtime 物理路径可按 F1 迁移，不能因保值规则被误解为必须永远使用旧路径。

legacy v2 target 数据库意图必须分为：

- `bootstrap_database_url`：指向 target MySQL 8.4 服务器上已存在的 system/bootstrap 库，凭据仅供
  lifecycle 在最终确认后创建 target 库、应用 principal 和授权；
- `application_database_url`：指定最终受限 runtime principal 和期望数据库名；该库在第一笔 mutation
  前必须不存在，未来 apply 创建后只有这个 URL 可物化为 native runtime `database_url`；
- `application_account_host`：固定 future MySQL `CREATE USER`/`GRANT` 的 account host，表示 API/worker
  客户端的允许来源范围，而不是 DSN 中的 MySQL 服务器 host；只接受精确 hostname/IP 或规范 IPv4
  CIDR，禁止 `%`/`_` wildcard，缺失时不得默认为 `%`；
- `require_database_absent=true`：强制防止接管或覆盖已有库；仅有同一 operation 的 durable pending
  journal 能证明该库由它早先创建时，才可 resume/recover；
- `require_account_absent=true`：同样要求精确 application `'user'@'host'` 不存在；future create 不得使用
  `IF NOT EXISTS`，只有同一 operation journal 绑定的 pending principal 可恢复。

两个 target URL 必须是同一 host/port、不同库名和不同凭据，并使用受验证 TLS。操作者不得
手工预建 application database 或 account。bootstrap 凭据是 F1 lifecycle secret，不得进入最终 runtime file、报告或日志；
数据库创建完成并校验应用授权后应当撤销或脱离运行环境。应用 principal 只能获得新库运行所需的
最小权限，并限定到显式 `application_account_host`；不得用 `%` 作为未填写时的兼容默认。独立的
`lifecycle_audit_key` 必须把原始 secret manifest bytes 以 HMAC 形式纳入最终 report SHA；不得与 APP_KEY、
server token 或 target datastore 密码共用，也不得输出或物化进 runtime。target Redis
不存在对等的“创建逻辑
DB”；spec 指定的 DB/namespace 必须独立且为空，不得通过 `FLUSHDB` 消除非空事实。

当前 v2 spec 的 `runtime` 是 **file-only AppConfig key 集合**，不是全部部署配置。database pool 的
min/max/acquire/idle/lifetime、worker heartbeat/shutdown/cleanup/retention，以及 runtime/rules/frontend
path bootstrap 等仍由部署环境控制，尚未纳入 v2 的 typed materialization、provenance 和原子 promote。
在这些值进入 lifecycle plan 并完成语义校验前，config apply 必须继续视为 blocker；不得把
`materialized_runtime_config` helper 或一份通过 `validate` 的 JSON 称为已生成可启动的最终配置。

缺失 native config 对全新安装可以表示尚未初始化；对 legacy migration/native upgrade 必须 fail closed，
不能把缺失文件解释成空对象后用新默认值启动。

### 11.2 F1 secret 与路径

- `APP_KEY`；
- database/Redis/SMTP/reCAPTCHA/Telegram/payment/server credentials；
- secure/admin path、app URL、subscribe URL/path；
- trusted proxy/CORS boundary；
- runtime root、config path、rules path、frontend release root。

轮换要求：old/new 双接受或明确 maintenance window、受影响消费者清单、外部重注册、fingerprint 验证、
回滚步骤、审计记录。secret 不能出现在计划、日志、diff 或错误消息中；只能输出指纹或 redacted 状态。
这里的安全 fingerprint 必须是 secret-manager 的不透明 version ID，或用独立 audit key 计算的 HMAC；
不得输出低熵 secret 的裸 SHA hash。

### 11.3 `/var/lib/v2board`

native mutable state root 的语义为 F1，内容为 F0/F1：

- `config/config.json`；
- `rules/custom.*`；
- 后续受支持的 installation/upgrade runtime state。

生产镜像 UID/GID 当前为 `10001:10001`。UID、GID、mount point 或文件权限变化必须先在副本验证，
再原子迁移；不得因为新容器不可写而创建一个空的替代目录。

配置更新必须继续使用跨进程锁、`0600` 临时文件、fsync 和原子 rename，不得由升级器用普通覆盖写入。

### 11.4 旧前端定制的处置

站点文字、logo、background 和可安全迁移的 custom HTML 属于 operator state，必须保留或阻止切流。

旧 theme、Ant/Bootstrap/OneUI 资产、`custom.css`、`custom.js` 和打包 bundle 不进入新运行时。它们的
存在必须进入迁移报告并要求操作者确认“仅视觉/脚本定制不迁移”；不得偷偷执行旧脚本，也不得声称
已经保留其效果。

但在 inventory 和所有者明确接受功能损失之前，operator theme/custom CSS/custom JS 是 F2 unsupported
operator state，不是 F3。实际旧生产镜像 digest、它的配置快照和 asset inventory 在 pre-contract
回滚窗口内也是 F2 rollback artifact。只有旧部署不再是受支持回滚目标后，其 PHP/Composer runtime
才能降为 F3。pinned reference 永远只作证据/fixture，绝不是生产回滚来源。

## 12. Worker、fencing、维护窗口与并发

### F0

- 同一业务任务在升级前后最多产生一次业务结果；
- 已应答的流量、支付、邮件和 mutation 必须已经具有 durable recovery path；
- pending/leased work 不得被 cleanup 或迁移删除；
- scheduler single-owner、fencing、流量 reset barrier 和 business transaction boundary 的安全结果；
- `scheduled_traffic_reset_key` 等业务日防重标记和单调 epoch，避免升级当天重复清零；
- API、worker、schema 和 config 必须属于兼容的同一 release manifest。

具体 lease key/token 是临时实现状态，但 active lease 永远不能被升级器删除；只有 owner 已确认停止且
TTL 结束后才可重建。F0 是单 owner、fencing 和不重复业务结果。持久 job/outbox/work item 必须能绑定
installation UUID、producer release、payload/queue epoch 和兼容 worker 范围；worker 遇到过新、过旧或
错误 installation 的 payload 必须 fail closed，不得猜格式执行。

旧版迁移必须按顺序：

1. 停止创建新的旧 queue work；
2. 停止旧 scheduler/Horizon 取得新任务；
3. 排空、转换或列出所有 pending work；
4. 获得数据库 writer fence；
5. 证明没有旧 PHP API/worker/scheduler writer；
6. 才能开始会破坏旧 writer 的 schema/data 变更；
7. 切流前验证 native worker 的 lease、outbox 和 reconciliation 状态。

原生升级的 contract/drop 阶段同样必须证明所有低版本 writer 已退出。仅依赖 migration job 串行不能
阻止旧 API 或 worker 同时写入。

有效 writer fence 必须由旧 writer 无法绕过的机制实施，例如撤销旧数据库写凭据、数据库 deployment
epoch 检查、数据库只读权限或网络策略隔离。观察到进程退出、Redis scheduler lock 或“当前没有请求”
都不能单独构成 fence。fence proof 必须使用旧 API/worker 原凭据实际尝试 mutation 并在 commit 前失败，
还要覆盖旧主机恢复网络和进程管理器自动拉起。worker termination grace 不得短于配置的 shutdown
deadline，active jobs 必须完成或进入 durable recovery。

schema/config migration 只能由独立、串行的一次性 lifecycle job 执行，不得由普通 API/worker startup
自动执行。生产顺序固定为：fence/drain → backup proof → migration/bridge → verification → API
readiness → worker start → traffic cutover；migration job 使用的 config、secret 和 migration artifact
必须与目标 release 完全一致。

### 12.1 Legacy Redis ownership 与可丢弃边界

Redis key 归属不能靠 suffix 猜。future classifier 必须从已验证 effective config 得到：

- `P = REDIS_PREFIX`；direct Redis、RedisQueue 使用 default DB，物理 key 为 `P + logical`；
- `CP = P + (CACHE_PREFIX 为空 ? "" : CACHE_PREFIX + ":")`；`Cache::`/RateLimiter/scheduler mutex 使用
  cache DB。旧值末尾已有 `:` 时仍要再追加一个，不能擅自 normalize；
- `H = HORIZON_PREFIX`；Horizon 使用 default DB 但覆盖而不是拼接 `P`；
- Redis URL 中的 `/db` 对 component database 的 pinned Laravel precedence、default/cache 可能同一 DB、
  namespace 重叠时最长前缀优先，均必须按 reference/Laravel fixture 证明。

source spec 必须明确 `shared|exclusive` scope。shared 中 `P/CP/H` 外 key 只做 foreign inventory；exclusive
中任何额外 key 阻断。shared 且 prefix 为空、重叠或无法唯一证明 ownership 时直接阻断。任何 owned
namespace 内未分类 key、已知 key 的 TYPE 不符、SCAN 超限或无法无损读取 key 都是 F0 blocker。

固定分类如下：

- `P+v2board_{upload,download}_traffic` hash 是未落库流量；非空阻断。`P+traffic_reset_lock` 存在时阻断；
- `P+queues:<q>` list、`:delayed`/`:reserved` zset 是 durable work，必须清零；`:notify` 只是 wake token，
  真实队列清零后可丢。未知 owned queue 名仍阻断；
- `CP+USER_SESSIONS_*` 和 JWT auth cache 只因本安装固定 `logout_all` 才可在 fence 后丢弃；不得把任意
  随机 key 猜成 session；
- `CP+otp_*`/`otpn_*`/`totp_*` 服从订阅 token drain/window；注册限流、邮件验证码、节点在线、统计、
  scheduler mutex 等已具名 cache 可在 producer fence 后按 `discard_ephemeral_after_fence` 丢弃；
- `H+failed_jobs` 及其 status=failed job hash 仍可 retry，是 durable failed work，必须与 MySQL
  `failed_jobs` 一并清零或显式处置；其余 Horizon process/metrics/history 只有在 Horizon/worker/真实队列
  全部停止后才可丢弃。必须记录实际 Composer/Laravel/Horizon version，不能假设内部 key 枚举永远固定。

当前 bounded preflight 尚未实现此 ownership classifier；它对 namespace 外的 queue/traffic/lock/token
候选采用保守阻断，对其余 source key 仅 warning，并把 classifier 缺失列入 `implementation_blockers`。
因此当前不会把不完整 Redis proof 报成可操作的 `compatible` 或 `ready_for_confirmation`。

## 13. 发布单元、readiness、前端资产与回滚

### 13.1 Build manifest 与 deployment binding

每个 release 必须携带 installation-independent、内容不可变的 build manifest，供同一镜像安全部署到
多个 installation。它至少声明：

- application/build version、source commit、API/worker image digest 和 migration artifact checksum；
- 支持的 schema/config/data epoch 范围；
- 支持的 Redis key、queue/outbox/job payload、worker lease/protocol epoch 范围；
- minimum/maximum reader 和 writer epoch；
- 支持的 upgrade source 与 expand/backfill/cutover/contract capability；
- MySQL/Redis capability 和 migration privilege；
- 是否需要维护窗口、预估锁/空间和 backup requirement；
- 可回滚到的最低 release；
- user/admin frontend content ID、完整 asset manifest 和 retention requirement；
- 允许的 old/new API、worker、frontend 组合。

每个 installation 另有受审计的 deployment binding，至少绑定 installation UUID、选中的 build-manifest
digest、DB/Redis/runtime identity、当前 schema/config/data/Redis/job epoch、operation/phase、实际 secret
manager version/HMAC fingerprint、active frontend content ID 和 release pointer。历史 binding 是 F0；active
pointer 和 secret reference 按 F1/F2 受控切换，不能写回通用 build manifest。

readiness 必须联合验证 build manifest 与 deployment binding：本 binary 明确支持当前 installation 的
schema/config/data/Redis/job phase，且已知 migration checksum 为正确前缀。不得继续只要求“数据库
migration 数量与本 binary 完全相等”，也不得简单放宽为忽略未来 schema。too-old、too-new、checksum
drift、错误 installation UUID 和 API/worker release mismatch 必须有负例。

在兼容 manifest 和 N/N-1 测试真正落地前，现有 exact-ledger readiness 只能声明为维护窗口/lockstep
升级，不能对外宣称支持无停机 rolling upgrade。

### 13.2 前端资产 F2

旧 HTML 可能在发布后继续请求旧 hashed JS/CSS/font/image。整个兼容和回滚窗口内：

- 任一新旧 pod/CDN 都必须能提供受支持旧 HTML 引用的资产；
- 资产文件内容不可变，cache header 保持 immutable；
- 发布不得固定只保留两代后立即删除更早仍在窗口内的 release；
- 删除前必须证明没有受支持 release 或回滚目标引用它，并且所有者批准的最大 asset compatibility/
  retention window 已结束。匿名长期打开页面通常无法被证明为零；访问日志只能辅助，不能替代窗口。

可以使用按 release ID 保留的对象存储/CDN，或让每个镜像同时携带所有受支持资产；不能依赖单个
本地 volume 的 `previous` symlink 来证明跨 pod 滚动安全。

proof 必须覆盖旧 HTML 的 entry、dynamic chunks、CSS 中的 font/image URL 经负载均衡命中新 pod。

### 13.3 一致备份集

同一 operation fence 下的备份至少包含：

- MySQL snapshot、GTID/binlog/PITR 坐标和 migration/operation ledger；
- runtime config、custom rules、installation state 及配置锁协调后的 version；
- secret-manager version ID 与安全 fingerprint，不把明文 secret 复制到报告；
- 若承诺 session 连续性，Redis RDB/AOF 坐标、namespace/key schema 和 TTL inventory；
- API/worker image digest、release manifest、frontend release 和 asset inventory；
- 支付、邮件、Telegram、节点等维护窗口内新事件的 replay/reconciliation 起点。

备份必须加密、位于不同故障域，有明确 retention、RPO/RTO 和访问审计。restore drill 必须在禁止外部
网络副作用的隔离环境执行，默认禁用 worker、邮件、支付、Telegram 和节点发送。restore/upgrade 审计
还必须写入不会随业务数据库快照一起回退的外部 append-only 审计存储。

### 13.4 回滚 F0/F1

- 代码回滚只有在旧 binary 声明能读写当前 schema/config phase 时才允许；
- expand 后但 contract 前，可以按 manifest 回滚代码；
- backup 创建成功不等于可恢复，必须在隔离环境执行真实 restore drill 并通过校验；
- **接纳新写入前**：writer fence 尚未解除且确认没有新版本业务写入时，可以恢复同一 operation 的
  数据库、runtime、Redis 必要状态和 secret reference 一致快照；
- **接纳新写入后**：禁止直接恢复旧快照。必须用 PITR/binlog、durable mutation journal 或 forward
  recovery 保留切流后的写入，并对 session/traffic/credential epoch 做单调 reconciliation；
- 无法证明不丢新写入、不复活旧凭据时，只能 forward recovery；不能把数据损失包装成回滚；
- externally revoked secret 不得因快照引用旧 version 而重新启用；Redis restore 必须区分 auth、
  lease、临时 token、metrics/cache；
- 回滚不得恢复已撤销 session/token/credential，也不得重复结算已处理 callback、mail 或 traffic。

## 14. 破坏性升级固定协议

任何删列、删表、收紧类型/NULL、改变单位/枚举、改变 key/URL、替换 secret、修改外部 payload 或停止
旧 writer 支持的升级，都必须遵循：

1. **Inspect**：只读分类来源、数据规模、消费者和 drift，且必须零写入；
2. **Plan**：输出不含 secret 的逐步计划、兼容矩阵、空间/锁预算、backup/rollback requirement；
3. **Journal**：第一笔 mutation 前写入 pending operation、source/target fingerprint、plan checksum 和
   backup reference；
4. **Backup + restore proof**：创建一致快照/PITR 起点并在隔离环境恢复验证；
5. **Expand**：添加 nullable column/new table/new endpoint，不破坏旧 reader/writer；
6. **Bridge**：双读/双写或旧 writer trigger，且具有一致性检查；
7. **Backfill**：小批量、可限速、可暂停、可 resume，每批有 checkpoint；
8. **Verify**：ID 集合、关系、聚合、外部行为和 drift 全部通过；
9. **Cutover**：显式切换 reader/writer/feature flag；
10. **Observe**：覆盖已声明的回滚窗口和所有**有界 F2** 消费者窗口；永久支付验签历史等 F0 不因
    观察时间经过而退役；
11. **Contract**：证明低版本 writer 和对应旧消费者为零后才 drop/retire；
12. **Final verify**：更新 minimum reader/writer，完成 lineage promote，并记录 operation completed。

任何阶段失败都必须停在可识别状态；不得自动跨过失败步骤，也不得把“继续启动”当作恢复。

唯一 legacy migration 在这个通用协议上再冻结两个独立的人工决策点：

1. 旧系统仍运行时，`provision inspect` 做在线只读兼容检查。只有 verdict 为 `compatible`
   才可请操作者确认**是否进入维护窗口**；有 blocker 时只报告，这一步绝不是迁移授权。
2. 进入维护后 fence source API writer/worker/scheduler/node reporter，停止临时链接签发，结清并对账
   Redis 流量/队列，建立一致 backup/PITR 起点并完成隔离 restore proof。
3. `provision plan` 在 fence 后重做最终只读检查。只有 verdict 为 `ready_for_confirmation`，且已展示
   脱敏的 target 库/账号创建、转换、预计停机、proof 和 rollback 摘要，才可请第二次确认。
4. 最终确认必须绑定精确 `operation_id + report_sha256`。source/config/datastore/plan 任一变化、过期或重跑
   都使旧确认失效；不接受模糊 `yes`。
5. 只有第 4 步才能授权 future `provision apply` 写 pending journal、创建 target MySQL 并迁移。操作者
   拒绝时不创建 target 库；只有证明安全后才能解除 fence 并恢复旧系统。

当前 `provision inspect` 只实现 scope `online_read_only_compatibility_inspection` 的有界在线只读子集，
verdict 为 `compatible|blocked`；`provision plan` 只实现 scope `fenced_read_only_final_plan` 的有界最终
只读子集，verdict 为 `ready_for_confirmation|blocked`。两者读取部分 MySQL schema/data inventory、旧
Redis traffic/queue/token inventory、MySQL `server_uuid` / Redis `run_id` 实例隔离、target bootstrap
MySQL 能力、期望库名与应用账号不存在，以及 target Redis 为空；
不生成完整 mutation 步骤、兼容矩阵、空间/锁预算、可验证 backup/rollback plan 或 journal。其 attestation
仍只是操作者声明；其中旧 cache driver、Redis prefix、订阅 method/expire/停止签发时间也尚未由旧配置的
安全解析器证明。未分类 source Redis key 目前只形成 warning，不代表已证明可丢弃。这些静态能力缺口
必须进入独立 `implementation_blockers`，在线阶段同样产生 `blocked/resolve_blockers` 与非零退出码；不能
伪装成可操作的 `compatible`。当前 `apply_available=false` 也在 verdict 状态机中直接禁止
`ready_for_confirmation`。只有所有 implementation proof 关闭且 `apply_available=true` 后，在线检查才可
输出 `compatible/confirm_enter_maintenance`。

## 15. 各路径必须满足的额外规则

### 15.1 全新安装

- 只接受严格 `empty`；发现任何业务表、ledger 或残留 installation state 即拒绝；
- 先建立可 resume 的 pending installation journal，并只生成一次 installation UUID；重试必须用同一
  operation 继续，不能在半安装状态重新生成 identity/secret；
- 要求或生成强 `APP_KEY`、server token 和管理员凭据，secret 不打印到日志；
- 创建 current native schema，不创建旧版 schema 再逐级升级；
- 在单个 DB transaction 内创建首个管理员和 DB installation record；跨 DB、Redis、runtime file、
  secret manager 的整体流程使用 staged operation、临时配置和最后 promote，不能伪称一个 ACID transaction；
- 不创建本地测试套餐、知识、固定管理员密码或其他 seed；
- 直接使用 scoped node token、幂等 traffic report、current Stripe metadata 和 current Telegram secret；
- 完成 config、DB、Redis、runtime PVC 和 frontend readiness 后才原子标记 active；失败状态不得接流量；
- 管理员 secret 只通过 stdin、secret file/manager 或一次性受限 bootstrap token 交付，不进入参数列表、
  日志或 shell history。

### 15.2 唯一旧版迁移

- 只接受 `legacy-reference-supported`；
- 原值保留所有 F0 数据和 F1 secret/path；
- 对旧 `.env`、`config/v2board.php`、theme/operator state 和 custom rules 只做 inventory；目标值全部按
  `manual_only` spec 手填，旧 theme/custom artifact 只能显式替代或经确认放弃；
- 自动检查 MySQL source 和 target bootstrap 服务器版本/能力，不假设可原地从 5.7 升到 current target；
- spec 必须提供 `bootstrap_database_url`、`application_database_url`、明确且不宽泛默认的
  `application_account_host`、`require_database_absent=true`、`require_account_absent=true` 和独立
  `lifecycle_audit_key`；只读检查证明期望库名及精确应用 `'user'@'host'` 不存在，操作者不手工预建；
- 在线 `inspect` 通过后只请求确认是否进入维护；fence/drain/backup/restore proof 与最终
  `plan` 全部通过后，展示脱敏摘要并要求绑定精确 `operation_id + report_sha256` 的第二次确认；
- future apply 只能在第二次确认后，先写 pending journal，再使用 bootstrap 凭据创建
  `utf8mb4/utf8mb4_unicode_ci` 新库、按 `application_account_host` 用无 `IF NOT EXISTS` 的语句创建
  最小权限应用账号和
  native schema；bootstrap 凭据不写入 runtime；
- target Redis 只验证选定逻辑 DB/namespace 为空；不创建逻辑 DB，不用 `FLUSHDB` 清理；
- 当前 legacy adapter 只接受检测为 standalone 的 MySQL/Redis：MySQL UUID 有效、非零、不同且无已检测
  replication/group/binlog-replica 拓扑；Redis 为 master、零 connected replica、cluster disabled 且 run_id
  不同。未注册/离线 replica 和底层 storage domain 仍须 future topology proof，不能靠单节点 identity 放行；
- fencing 旧 PHP/API/Horizon/scheduler；
- 固定 `logout_all`，不复制旧 session；固定 `maintenance_cutover`，停止全部 node reporter 后再配置
  scoped token 与稳定 idempotency key；重注册 Telegram，并按 `assert_none` 自动核验 Stripe inventory；
- 运行具名、可 resume 的 legacy bridge；
- 第一次 mutation 前创建 pending operation journal；每个持久步骤单调 checkpoint；
- legacy bridge 中每个可能隐式提交的持久 DDL 都必须独立记录和校验；不得把当前开发期早期
  multi-DDL migration 不加保护地直接当成可恢复生产步骤；
- legacy bridge 完成并验证后才 promote installation 为 native lineage；不得等到完成后才首次记录
  operation；
- 完成前后 proof obligations 后才切流；
- 实际旧生产镜像只有在 schema/config 仍兼容且 manifest 允许时才是 pre-contract 回滚目标；pinned
  reference 永不用于运行或回滚。旧 writer 不与 native 同时写入。

### 15.3 Native 非破坏性升级

- 只接受 `native`；
- 已知 migration checksum 前缀正确；
- release manifest 声明兼容当前 schema/config；
- config 新默认不得改变既有有效值；
- API/worker/frontend 可以滚动混跑时，必须通过 N/N-1 reader/writer 测试；
- 保留回滚窗口所需 schema bridge 和前端资产。

### 15.4 Native 破坏性升级

- 除 15.3 外，完整执行第 14 节；
- contract 前必须单独获得所有者批准；
- 无真实 restore proof、无消费者 drain proof、无低版本 writer fencing proof 时不得 contract；
- down migration 不能替代一致快照恢复；
- 破坏性升级不得自动运行在普通容器启动路径中。

## 16. Proof obligations

“迁移完成”至少需要以下证据。工具应输出机器可读结果，CI 应保存不含 secret 的报告。

### 16.1 来源与 schema

- empty/reference-supported/reference-drift/native/recoverable-pending/unknown 分类 fixture 全覆盖；
- reference commit 和完整 schema fingerprint；
- MySQL version、charset、collation、sql mode、DDL capability、migration privilege；
- migration/bridge ledger、checksum、真实 schema drift；
- 分类失败、错误目标 DB 和 installation mismatch 必须证明零写入；
- 每个 DDL/DML/backfill checkpoint 故障注入后的安全 resume；
- 大表锁时间、额外磁盘和复制延迟预算。

### 16.2 数据

- 所有业务表 row count、完整主键/自然键集合、按 PK canonical row hash 和 next auto-increment；
- 用户 token/UUID/password hash/role/epoch 的逐值摘要；
- order/payment/callback/reconciliation binding；
- 金额以 DECIMAL/i128 比较逐列总和、NULL/负数数量，并按 status/type/commission/provider/time window
  分桶；不得经过浮点；
- 用户与统计流量使用精确整数比较 u/d/quota 总和及完整 rate/coercion/rounding 样本；
- coupon/giftcard/invite redemption 和使用次数；
- plan/group/node/route、ticket/message 等关系完整性；
- pending/leased/terminal outbox、traffic report 和 worker work inventory；
- legacy `v2board_upload_traffic`/`v2board_download_traffic` 与 TrafficFetch/StatUser/StatServer inventory、
  drain 前后 DB/stat 对账；
- 所有 legacy→native 新字段的初始化集合，尤其 session/traffic/credential epoch、payment archived、
  callback digest 和 scheduled reset day；
- 所有状态机数量和 NULL/zero/sentinel 数量，尤其 `plan_id=0`、`ticket_message.user_id=0`、
  expired/price/limit 的 NULL 与 0、reconciliation status -1；
- malformed JSON、目标 collation 唯一碰撞、FK orphan 和重复未完成 order/open ticket 均 fail closed。

### 16.3 配置和 secret

- APP_KEY、server token、payment/SMTP/Telegram/reCAPTCHA 等只比较安全 fingerprint；
- 迁移前后 effective config typed snapshot，不包含明文 secret；
- environment/file/default/secret-reference provenance 和优先级一致；
- 缺失 config、未知 key、unsupported custom file 必须 fail closed；
- `/var/lib/v2board` PVC 对 UID/GID 10001 的读写和原子 rename；
- Redis 逐 key 比较 type、namespace、TTL、value semantic hash 和数量；
- DB/runtime/Redis/secret installation UUID 任一不匹配均拒绝启动；
- 原始 secret 不出现在日志、错误、JSON 报告、进程参数或容器 inspect；
- custom rules parse + 每种对应输出 render。

### 16.4 外部行为

- API 逐 endpoint route/method/encoding/status/envelope/error contract；
- 用户/admin Hash route 和从 pinned 旧版**实际 producer 调用链**提取的旧链接；模板文件存在本身不算；
- auth storage、locale、`logout_all` 安全撤销；fixture 覆盖普通/admin/封禁/已删除用户，证明旧 JWT/session
  一律拒绝且 admin 不继承 step-up；本安装不要求 legacy serializer conversion fixture；
- 订阅完整有限矩阵：每个 UA、flag、format、header profile、token method 0/1/2、OTP/TOTP mapping/TTL、
  import URI、`getConfig/getVersion` 分支和 inventory 中每个旧 host/path；
- 每个 payment provider 的 outbound checkout 与 signed raw callback fixture：签名 header、金额、币种、
  exact ACK body/status/content-type、重放和迟到 callback；
- pre-cutover Stripe 处置清单为零或全部 reconciliation；
- Telegram `setWebhook` + `getWebhookInfo`；
- 每节点 token/idempotency adoption 和最后 legacy request；
- Crisp/Tawk identity/session payload；
- old/new pod 下旧 HTML hashed asset 请求。

### 16.5 运维和回滚

- 旧 writer、Horizon、scheduler 已 fencing；
- fence 后旧凭据实际 mutation 失败，并覆盖旧主机恢复网络/自动拉起；
- API/worker/frontend/schema/config build identity 一致；
- API/worker/schema/config/Redis/job 的全部允许与拒绝兼容矩阵；
- `/readyz` 和 worker health 之外的业务 smoke；
- 一致 backup 已在隔离环境真实 restore；
- code-only rollback 是否允许的机器判定；
- frontend promote/rollback、跨 pod lazy asset；
- snapshot restore 后新写入 replay、epoch 单调 reconciliation，且演练无真实外部副作用；
- 每类可重建 F3 删除后可从锁文件和 release artifact 完整重建；每类“明确不迁移”F3 有 owner
  acceptance、inventory 和回滚窗口结束证据；
- contract 后完整 restore/forward-recovery 演练。

## 17. 明确可重建或不迁移的状态

只有本节及未来经所有者明确追加的项目属于 F3：

- Cargo registry/git cache、Rust `target`；
- pnpm store、`node_modules`、Vite/build/test/cache/report 目录；
- Playwright 浏览器和视觉/交互报告；
- worker health timestamp file；
- 可重新计算且不承担幂等、会话、限流安全或业务恢复职责的 metrics/cache；
- 在实际旧生产镜像、配置和资产 inventory 的 F2 回滚窗口结束后，本地旧 PHP/Composer 依赖和
  PHP runtime copy；
- 在没有受支持旧 HTML/回滚目标引用且 asset retention 到期后，旧 Ant/Bootstrap/OneUI bundle、
  固定 `umi.js`/`umi.css` 的冗余 copy；
- 经 inventory、所有者具名接受功能损失且回滚窗口结束的旧 theme/custom CSS/custom JS unsupported
  operator artifact。

以下**不属于 F3**：MySQL 数据、`_sqlx_migrations`、installation/upgrade ledger、runtime config、
custom subscription rules、payment verification history、native Redis session、JWT cutoff、idempotency ledger、
pending/leased work、mail outbox、reconciliation、回滚窗口内的 hashed frontend assets。唯一例外是本次
已具名选择 `logout_all` 的旧 Laravel session 与经 fencing 后确认可重建的旧 cache；它们不复制，但不得
通过整库 flush 顺带删除未结流量、queue、订阅 token 或未知 durable key。

`make reset`、`docker compose down -v`、整库 Redis flush 和删除 runtime PVC 都是销毁操作，绝不能出现在
安装迁移或普通升级路径。

## 18. 变更控制

1. 所有者复查本文时，可以逐条修改保护等级或补充例外；未明确修改的条目继续冻结。
2. 本文通过复查后，任何降低保护等级、缩短兼容窗口或新增可丢弃项的变更必须单独提交，说明真实
   消费者、数据损失风险、回滚影响和替代方案。
3. 任何实现不得以“代码已经这样做”为理由降低本文要求；发现实现与本文冲突时，默认实现是缺陷。
4. 新增外部接口、持久化字段、secret、Redis key、后台任务或 runtime 文件时，必须在同一变更中将其
   加入本文并指定等级与 proof obligation。
5. 行为/interaction tests 是最低自动化证据；F0/F1 契约应尽可能有专门的 contract test，不能仅靠
   文档记忆。

## 19. 当前实现符合性基线

本表防止“规范已经写了”被误读为“代码已经支持”。只有同时达到 implemented、tested、restore-drilled
且 proof 归档，才能把对应路径标记 supported。

| 能力 | 当前状态 | 生产结论 |
| --- | --- | --- |
| lifecycle JSON v2 文件校验 | 部分实现：同一 file descriptor 校验 regular/non-symlink、大小/Unix 权限，拒绝 duplicate/unknown/missing key，验证完整 file-only AppConfig、共享 runtime 语义、TLS、解码后的 target bootstrap/application 连接意图、精确 MySQL account host、独立 lifecycle audit HMAC binding、固定路径及 `manual_only`/`logout_all`/`discard_ephemeral_after_fence`/`maintenance_cutover` 等固定决策 | 能证明手填 spec 在当前 v2 静态语义下有效；尚未覆盖 pool/worker/path bootstrap、旧 artifact inventory、installation binding 或原子 promote，不能据此启动迁移 |
| bounded online `inspect` / fenced final `plan` | 部分实现：固定 reference 声明、有限 core schema profile、具名 DB/Redis/data blocker、有效非零 MySQL `server_uuid` 与 Redis `run_id` process identity、已检测 MySQL replication/group 与 Redis replica/cluster standalone gate、target MySQL 8.4 bootstrap 能力/期望库名和账号不存在与空 Redis target 检查；MySQL 版本由服务器探测 | 未实现能力会显式列入 `implementation_blockers`，所以当前在线/final 均安全返回 blocked；未注册/离线 replica 与 storage domain、旧 cache/prefix/subscription 声明、未分类 Redis key、完整 Stripe callback/reconciliation proof 仍未机器验证，不是完整来源分类、lineage/installation binding、§14 Plan 或迁移许可，`apply_available=false` |
| 只读完整来源分类与 installation binding | 未实现 | 不支持自动选择安装/迁移路径；当前 preflight 不能证明 `legacy-reference-supported` |
| staged fresh install 与首个管理员 bootstrap | 未实现 | 不支持生产 fresh-install workflow |
| pinned legacy config/session/data/payment/node bridge | 未实现 | 不支持从旧版直接切流 |
| runtime config materialize/stage/atomic promote | helper/读取 fallback 部分存在，但没有 lifecycle apply | v2 未覆盖 pool/worker/path bootstrap，CLI 不写 `config.json`；config apply 仍是 blocker |
| `provision apply` / target create / data copy / cutover | 未实现 | 没有写入命令；CLI 不创建 target 库或账号，不得把 validate/inspect/plan 后的手工操作称为受支持迁移 |
| native SQLx migration | 已有显式 CLI/本地 one-shot，能力仍部分 | 只能视为开发期 schema runner；缺来源分类、installation binding、operation journal/fault resume，`0001 IF NOT EXISTS` 不能接管旧库，exact readiness 不支持 rolling |
| pending operation journal/checkpoint/fault resume | 未实现 | 任一破坏性写入均不得执行 |
| release compatibility manifest 与 N/N-1 matrix | 未实现 | 只允许未来维护窗口/lockstep 设计，不支持 rolling 声明 |
| 一致 backup/PITR/isolated restore drill | 未实现或未归档 | 不支持 contract/drop 或数据转换 |
| 外部 contract/full subscription/provider/node fixtures | 部分已有 parity | 不足以证明迁移完成 |

每次实现推进必须在同一变更更新此表并链接机器 proof；口头确认或手工成功一次不能升级状态。

## 20. 代码证据入口

- 唯一旧版 schema：[`references/wyx2685-v2board/database/install.sql`](../references/wyx2685-v2board/database/install.sql)
- 旧版逐步更新 SQL：[`references/wyx2685-v2board/database/update.sql`](../references/wyx2685-v2board/database/update.sql)
- 旧版安装/更新行为：[`V2boardInstall.php`](../references/wyx2685-v2board/app/Console/Commands/V2boardInstall.php)、
  [`V2boardUpdate.php`](../references/wyx2685-v2board/app/Console/Commands/V2boardUpdate.php)
- current CLI grammar：[`backend/rust/crates/api/src/cli.rs`](../backend/rust/crates/api/src/cli.rs)
- lifecycle spec 与 bounded read-only preflight：[`backend/rust/crates/provision`](../backend/rust/crates/provision)
- 不可直接运行的 v2 手填示例：[`docs/examples/legacy-migration.v2.example.json`](examples/legacy-migration.v2.example.json)
- v2 手填与双阶段只读检查说明：[`docs/legacy-migration-manifest.md`](legacy-migration-manifest.md)
- current migration runner：[`backend/rust/crates/db/src/pool.rs`](../backend/rust/crates/db/src/pool.rs)
- current schema migrations：[`backend/rust/migrations`](../backend/rust/migrations)
- runtime config：[`backend/rust/crates/config/src/lib.rs`](../backend/rust/crates/config/src/lib.rs)
- HTTP routes：[`backend/rust/crates/api/src/routes.rs`](../backend/rust/crates/api/src/routes.rs)
- authentication/session：[`backend/rust/crates/domain/src/auth`](../backend/rust/crates/domain/src/auth)
- order/payment：[`backend/rust/crates/domain/src/order`](../backend/rust/crates/domain/src/order)
- node API：[`backend/rust/crates/api/src/server_api`](../backend/rust/crates/api/src/server_api)
- frontend API contracts：[`frontend/packages/api-client/src`](../frontend/packages/api-client/src)
- user/admin routes：[`frontend/apps/user/src/App.tsx`](../frontend/apps/user/src/App.tsx)、
  [`frontend/apps/admin/src/App.tsx`](../frontend/apps/admin/src/App.tsx)
- compatibility scenarios：[`frontend/tests/lib/interaction-scenarios.mjs`](../frontend/tests/lib/interaction-scenarios.mjs)
