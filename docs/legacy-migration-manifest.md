# 唯一旧版迁移清单 v2

本清单只服务于 `references/wyx2685-v2board` 固定 commit
`7e77de9f4873b317157490529f7be7d6f8a62421`。采用一个手填 JSON 是正确方向，但边界必须清楚：

- **手填目标意图**：source 只读连接、target MySQL bootstrap 连接、期望的受限应用连接、
  target Redis 逻辑 DB、站点与安全配置、邮件/Telegram、节点 master secret 和迁移决策；
- **自动读取现场事实**：MySQL 实际版本与结构、旧数据冲突、未完成订单、Stripe inventory、节点数量、
  Redis 流量/队列/锁/临时订阅 token、source/target 实例身份、target MySQL 服务器能力、期望库名及
  期望应用账号不存在、target Redis 为空；
- **绝不把旧配置导入目标**：不合并或执行旧 `.env`、`config/v2board.php`、theme、custom PHP/JS；
- **不能手填可探测事实来绕过检查**：例如没有 MySQL version 字段，不能靠填写“旧库是 MySQL 8”
  跳过服务器探测，也不能只声明“没用 Stripe”跳过数据库零状态检查；
- **当前仍有未机器验证的来源声明**：cache driver、两个 Redis prefix、旧订阅方式/有效期和停止签发
  时间必须手填，当前工具不会读取旧 config 证明它们正确。它们只帮助执行有界预检；未来 apply 在
  安全解析器或等价机器证据落地前继续阻断。

因此，一个文件会比自动猜测旧配置更简单、可复查，但不会替代数据库和 Redis 的自动检查。

这里的 `manual_only` 专指“目标配置由操作者明确填写，不从旧配置自动转换”。只读验证少数来源事实不
等于导入目标配置；成熟 apply 必须补上这一步，不能永远信任人工填写。

## 已固定的选择

v2 不提供自由组合，以下值必须原样出现：

| 项目 | 固定值 | 结果 |
| --- | --- | --- |
| 旧配置 | `manual_only` | 目标值全部手填，不导入旧配置 |
| 登录态 | `logout_all` | 不转换 Laravel session；切换后所有用户和管理员重新登录 |
| 旧 cache | `discard_ephemeral_after_fence` | 停止旧 producer 后不复制登录态、限流和可重建 cache；不包含流量/队列 |
| Stripe | `assert_none` | 自动查库证明 Stripe payment 与相关未完成订单为零，否则阻断 |
| 临时订阅链接 | `assert_none` | 扫描 OTP/TOTP key，并等待旧签发方式的最长有效窗口结束 |
| 节点 | `maintenance_cutover` | 停止所有旧 reporter，切换后逐节点配置 scoped token 和幂等键 |
| 旧主题 | `discard_confirmed` | 不运行或复制旧主题、CSS、JS 和 bundle |
| 自定义订阅规则 | `none` 或 `discard_confirmed` | 只有实际不存在或明确接受放弃时才能选择 |

## MySQL 里的永久订阅 token 与 Redis 临时状态

用户的永久订阅凭据是 MySQL `v2_user.token`，不是 Redis key。它与用户 ID、密码 hash、
`uuid` 一样是 F0 数据：迁移必须保留原值，不得因为全量登出、清 cache 或升级而重生。
旧订阅方式 0 直接把这个永久 token 放入 URL。

Redis 中的 `otp_`/`otpn_`/`totp_` 是为了不在每个对外 URL 中直接暴露永久 token 而产生的
短期表示：

- 方式 1 使用 `otp_{permanent}` 和 `otpn_{temporary}` 两个 24 小时映射；订阅验证成功时
  反查 MySQL 永久 token 并消费该临时映射。
- 方式 2 由用户 ID、MySQL 永久 token 和当前时间窗计算 TOTP URL；生成 URL 时可以完全
  不写 Redis。第一次验证时先用用户 ID 从 MySQL 找回永久 token，然后才可能在 Redis 中
  以 `totp_{temporary}` 再缓存一个完整 timestep。

所以这些临时链接不适合写入 MySQL 长期迁移，而“Redis 扫不到 key”也不能单独证明外部已签发的
TOTP URL 已失效。必须先停止签发，再等完已验证的最长有效窗口。`TEMP_TOKEN`
快捷登录、邮件验证码、限流、节点在线状态、`USER_SESSIONS_*`/JWT 登录 cache 等也是 Redis 临时状态；按
`logout_all` 和 `discard_ephemeral_after_fence` 可在停止旧 producer 后放弃，但不能把未知 key
自动当成 cache。

另一类 Redis 数据不是 cache：节点已经上报、但定时任务还没累加到 MySQL `v2_user.u/d`
及统计表的上传/下载增量，会先存在 Redis `v2board_upload_traffic` /
`v2board_download_traffic` hash 中。这是“已接受、尚未落库”的权威流量，不是历史流量
报表的缓存；必须在 fence 后持久化、对账并证明为空，不得直接删除。真实 queue 和
可重试失败任务也同样不属于可丢 cache。

## 为什么旧 Redis 要填两个地址

固定旧版通常把直接 Redis 命令和队列放在 `REDIS_DB`（常见 DB 0），把 Laravel `Cache::` 放在
`REDIS_CACHE_DB`（常见 DB 1）。流量 hash 在前者，`otp_`/`otpn_`/`totp_` 在后者；两边还会叠加
`REDIS_PREFIX`，Cache key 另叠加 `CACHE_PREFIX`。只填写一个 Redis URL 会产生假零结果，所以 v2
分别要求：

- `redis_default_url`；
- `redis_cache_url`；
- `redis_connection_prefix`；
- `redis_cache_prefix`。

v2 只接受 `legacy_cache_driver=redis`。若真实旧安装使用 file、memcached 或其他 cache driver，不能填写
Redis 来伪装通过；应归为 unsupported source，先补对应的只读 inventory 适配器。

订阅方式 2 的 TOTP URL 在生成时不会写 Redis。为避免“扫描不到 key”被误判为不存在，还必须填写旧
`show_subscribe_method`、`show_subscribe_expire` 和停止签发时间；方式 1 至少等待 24 小时，方式 2
在只有“停止签发”人工时间而没有完整 runtime fence proof 时按两个旧 time bucket 等待，另加时钟裕量。

上述 cache driver、prefix、method、expire 目前都是 operator declaration；填错可能产生假零。即使报告
出现 `compatible` 或 `ready_for_confirmation`，也不能据此手工迁移。它们以及未分类的 source Redis key
都是未来 apply blocker；
当前 `discard_ephemeral_after_fence` 还没有机器证明每一个遗留 key 都确实可重建。

## 节点维护切换是什么意思

“节点”是部署在各服务器上的代理程序/reporters，不是数据库里的节点记录。旧 reporter 常共用一个
全局 token，而且同一批流量没有稳定的唯一编号；新版默认使用每节点凭据，并要求稳定的
`Idempotency-Key`，以防同一批流量重复计费。

所以这里不做在线兼容桥。维护时先停旧 reporter、结清 Redis 流量与队列，迁移完成后为每个节点生成
独立凭据，更新其面板地址/token/幂等上报能力，再逐个恢复。若直接切换而不更新，表现为配置拉取和
流量上报被拒绝，即“掉线”；节点表数据本身不会丢。

## Target MySQL 由工具创建，不要手工预建库

v2 的 target 明确分开两个 MySQL URL：

- `bootstrap_database_url`：连接新 MySQL 8.4 服务器上已存在的 system/bootstrap 库，使用仅供
  lifecycle 创建 target 库、应用账号和授权的管理凭据；
- `application_database_url`：最终 runtime 要使用的受限账号和期望库名；在检查及最终确认之前，
  该库名必须不存在。
- `application_account_host`：future `CREATE USER`/`GRANT` 中 MySQL account 的 host 范围（即
  `'application_user'@'application_account_host'`）；它是 API/worker 客户端来源范围，不是 DSN 中
  MySQL 服务器 host。只接受精确 hostname、精确 IP 或规范 IPv4 CIDR；禁止 `%`、`_` 等 MySQL
  wildcard，也不得默认成宽泛 `%`。

`require_database_absent=true` 与 `require_account_absent=true` 是防止误覆盖、误接管 principal 的强制
声明。两个 URL 必须指向同一 host/port、
不同数据库和不同凭据。在线检查只连 bootstrap URL：自动识别服务器版本/能力，并通过
`information_schema` 与 `mysql.user` 证明期望库名及精确 `'user'@'host'` 不存在；bootstrap principal
若无权读取这些证明信息就安全失败。不连接、不创建尚不存在的应用库。

只有在维护期最终检查全部通过，操作者又确认精确 `operation_id + report_sha256` 后，future
`provision apply` 才可用 bootstrap 凭据创建新库，固定为 `utf8mb4/utf8mb4_unicode_ci`，创建/限定
应用账号仅能从 `application_account_host` 登录并仅获得运行所需权限，然后创建 native schema。
future apply 必须使用不带 `IF NOT EXISTS` 的 `CREATE DATABASE` / `CREATE USER`，bootstrap 凭据绝不得
写入最终 `config.json`。如果期望库名或账号已存在，默认阻断；只有 durable journal 证明它就是同一
operation 之前创建的 pending target，才能进入 resume/recovery，不能复用无 lineage 的旧账号。

Redis 没有对等的“创建逻辑 DB”操作。操作者在 `redis_url` 选择 target DB number/namespace，工具只验证
它与 source 隔离、为空且具备必要命令；不会用 `FLUSHDB` 帮忙“变空”。需要新物理 Redis 实例时，
由基础设施先提供连接，lifecycle 工具仍只管该逻辑 DB/namespace。

## 先检查，再两次明确确认

唯一旧版迁移固定为以下流程：

1. `provision inspect` 在旧系统仍运行时执行在线只读兼容检查；有 blocker 就完整报告，不进入
   维护或迁移。
2. 报告为 `compatible` 时，只请操作者第一次确认：**是否进入维护窗口**；这不是确认迁移。
3. 确认后 fence 旧 API writer、worker、scheduler 和所有 node reporter，停止签发临时链接，安全结清/
   对账 Redis 流量与队列，建立一致备份并通过隔离 restore drill。
4. `provision plan` 在 fence 后重做最终只读检查。任何 source/config/datastore 变化都废弃旧报告；
   只有 `ready_for_confirmation` 才能展示最终数据库创建、转换、预计停机、proof 和回滚摘要。
5. 操作者第二次确认必须绑定精确 `operation_id + report_sha256`；过期、重跑或内容改变后的报告
   不能沿用旧确认。
6. 只有这个最终确认才允许 future `provision apply` 写 journal、创建 target MySQL 并迁移。拒绝确认时
   不创建 target 库；在证明安全后可解除 fence 并恢复旧系统。

当前 CLI 只做到第 1 和第 4 的有界只读报告，不会互动地进入维护，也没有第 6 的 `apply`。
两次确认是冻结的未来编排契约，不能用 shell 的模糊 `yes` 或手工执行 SQL 绕过。
journal、backup binding、source facts、Redis ownership、target 权限和 apply 等静态能力缺口会列入
`implementation_blockers`，在在线阶段也直接给出 `blocked/resolve_blockers`；不会用退出码 0 的
`compatible` 掩盖“迁移器尚未实现”。只有这些缺口关闭且 `apply_available=true` 后，在线检查才可返回
`compatible/confirm_enter_maintenance`。

## 使用当前只读命令

公开示例包含故意无效的 UUID、域名、CIDR、密钥和路径，也不是 secret 文件权限。复制到仓库外后必须
逐项复核并填写所有字段，不能只替换包含 `REPLACE` 的值：

```bash
cp docs/examples/legacy-migration.v2.example.json /secure/private/legacy-migration.json
uuidgen  # 把结果填入 operation_id，不要复用示例 UUID
chmod 600 /secure/private/legacy-migration.json
v2board-api provision validate --manifest /secure/private/legacy-migration.json
v2board-api provision inspect --manifest /secure/private/legacy-migration.json
# 仅在上面报告 compatible、你确认进入维护并完成 fence/drain/backup/restore proof 后：
v2board-api provision plan --manifest /secure/private/legacy-migration.json
```

`lifecycle_audit_key` 必须是新生成、至少 32 bytes、只供 lifecycle 审计使用的独立 secret；不得与
`runtime.app_key`、`runtime.server_token` 或 target datastore 密码相同，也不会物化进最终 runtime。报告只包含用它对**原始清单
完整 bytes**计算的 `manifest_binding_hmac_sha256`。因此 DSN、密码、backup reference、runtime 值乃至
空白格式发生任何变化，绑定值和最终 `report_sha256` 都会变化，旧确认立即失效；报告又不会暴露可用于
离线猜测清单中低熵 secret 的裸 manifest hash。

公开示例的 maintenance attestations 默认都是 `false`，`backup_reference` 默认是 `null`，这是在线
`inspect` 之前的正确初始状态。不得为了让命令变绿而提前填 `true`。只有第一次确认进入
维护、实际完成对应 fence/drain/backup/restore proof 后，才把每项 attestation 更新为真实值，
填入非占位的 backup reference，再执行最终 `plan`。

连接 URL 内的用户名、密码若含 `@`、`:`、`/`、`#` 等字符，必须 percent-encode；校验器会先严格解码
再检查用户名分离、长度和 placeholder，编码不能绕过规则。`app_url`、每个 `subscribe_url` 和
`server_api_url` 必须是无 userinfo/path/query/fragment 的规范 HTTPS origin；CORS entry 同样如此。
`verified_tls` 要求
source MySQL 使用 `ssl-mode=VERIFY_IDENTITY`，两个 source Redis 都使用 `rediss://`；旧基础设施确实只在
隔离维护网内可达时，才可显式选择 `trusted_maintenance_network`。

`validate` 不连接也不写入；`inspect` 是在线只读检查，scope 为
`online_read_only_compatibility_inspection`，verdict 为 `compatible|blocked`；`plan` 是 fence 后最终只读检查，
scope 为 `fenced_read_only_final_plan`，verdict 为 `ready_for_confirmation|blocked`。它们都输出脱敏
`operation_id`、`manifest_binding_hmac_sha256`、`report_sha256` 和 `apply_available=false`。
`report_sha256` 是将该字段置空后 canonical report payload 的摘要；payload 包含 manifest binding、
现场实例 identity 和所有检查结果，不是最终打印 JSON 文件的直接哈希。存在 blocker 时，命令在
完整打印 JSON 后返回非零退出码。

target MySQL 必须是可连接的 MySQL 8.4+ bootstrap 服务器，且 `application_database_url` 指定的库名
和解码后的应用 `'user'@'host'` 必须不存在；source/target MySQL `server_uuid` 也必须不同。将来创建时
固定使用 `utf8mb4/utf8mb4_unicode_ci`。当前 legacy 路径只接受没有检测到 replication channel、group
replication member 或 binlog replica client 的 standalone MySQL。target Redis 必须是与 source `run_id`
不同的空 Redis 6.2+ 逻辑 DB，并且 source/target 都必须报告 `role=master`、零 connected replica、
`cluster_enabled=0`；cluster/replica 拓扑会阻断，避免单节点 `SCAN` 产生假空。它还必须可见 `GETDEL`、
`EVALSHA`、`SCRIPT` 命令。命令存在不等于当前 ACL 真能执行，因此当前列为 implementation blocker。
这些探测仍不能证明未注册/离线 replica 或底层 storage failure domain；完整 topology binding 继续作为
implementation blocker，不能仅凭 UUID/run_id 宣称物理隔离完成。
每次 Redis SCAN 最多接受 100,000 个 key，超过即报错而不是给出不完整 inventory。

当前清单还没有 `shared|exclusive` Redis ownership 和独立 Horizon prefix 证明；全库发现 namespace 外的
queue/traffic/lock/OTP-like key 时会保守阻断，而其余未分类 key 只 warning。共享 Redis DB 因而可能产生
安全的假阳性，并未得到完整支持；未来 apply 必须按契约中的 `P/CP/H` classifier 关闭假阳性和假阴性。

当前 Stripe `assert_none` 只机器统计旧库中的 `Stripe*` payment row 及其 status 0/1 order。provider 侧
callback/reconciliation inventory 尚未证明，因此完整 Stripe proof 仍是 apply blocker；不能把当前
`compatible` 或 `ready_for_confirmation` 称为完整的 Stripe not-applicable 证明。

## 当前不能做的事

仓库没有 `provision apply`，报告固定输出 `apply_available=false`。当前不会创建 target MySQL、复制数据、运行 target
migration、写 `config.json`、生成节点凭据、建立 operation journal、验证 backup restore 或切流。
普通 `v2board-api migrate` 也已拒绝“有表但没有 native SQLx lineage”的数据库，绝不能指向旧库。

真正开放 apply 前仍必须补齐：最终确认后由 bootstrap 创建独立 MySQL 8.4 target、一致快照复制、
逐表数据 proof、可恢复 journal、
backup/restore drill、旧 runtime artifact inventory、pool/worker/path 配置纳入单一文件，以及最终原子
config promote。最终 `config.json` 只能物化 target runtime 值，绝不能保留 source 凭据、backup reference
或旧环境内容。旧 cache/prefix/subscription/queue 事实必须由严格、非执行式 parser 校验只读
`bootstrap/cache/config.php` snapshot 及 SHA，停止签发时间必须来自 future journal 的 machine fence，不能
继续信任当前手填值。完整不可变边界见 [安装、旧版迁移与升级不可变契约](upgrade-invariants.md)。
