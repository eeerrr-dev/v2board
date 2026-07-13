# V2Board Native

V2Board 的现代化实现：单一 Rust API、Rust worker，以及 React + TypeScript +
Vite + shadcn/ui 前端。PHP/Laravel 与旧打包前端不属于新版运行、构建或部署链路。

## 架构与数据所有权

- `backend/rust`：Axum API、后台任务、PostgreSQL migration、ClickHouse analytics
  projection 与本地开发 seed。
- `frontend/apps/user`：用户端 React 应用。
- `frontend/apps/admin`：管理端 React 应用。
- `frontend/packages`：类型、API client、i18n 与共享构建配置。
- `references/wyx2685-v2board`：固定且只读的旧版参考，只用于兼容审计和旧版迁移源识别。

新版数据存储固定为：

- **PostgreSQL 18**：唯一权威事务数据库。用户、订阅、额度、订单、支付、配置、幂等状态、
  migration/installation ledger 和待发布 analytics outbox 都以它为准。
- **ClickHouse 26.3 LTS**：只保存 PostgreSQL 先提交的不可变分析投影，绝不反写或决定核心业务状态。
- **Redis**：只保存 session、限流、lease、锁和短期缓存，不保存不可恢复账本。

MySQL 不是新版可选运行时。唯一允许的旧版数据库来源是
`references/wyx2685-v2board` 所代表、且精确识别为 Oracle MySQL 8.0/8.4 的**只读 source**；
MySQL 5.7、Percona 和 MariaDB 均不在迁移支持面内。
完整边界见 [PostgreSQL + ClickHouse 持久化不变量](docs/postgresql-clickhouse-invariants.md)。

Rust 在一个端口同时提供 API、用户页、动态后台路径和带内容哈希的静态资源。前端部署根包含
不可变 `releases/<content-id>/{user,admin}`，并通过 `current`/`previous` 原子链接切换版本。

## 安装、旧版迁移与升级

全新安装、旧版迁移、native 升级和破坏性升级都必须遵守
[安装、旧版迁移与升级不可变契约](docs/upgrade-invariants.md)以及
[PostgreSQL + ClickHouse 持久化不变量](docs/postgresql-clickhouse-invariants.md)。来源不明、
无法验证的半迁移或 installation identity 不一致时必须停止，不能猜测后继续写入。

当前一次性 lifecycle CLI 暴露检查、一次确认和同一 operation 的前向恢复命令：

- `v2board-lifecycle validate --manifest <path>`：严格校验 lifecycle JSON，不连接数据存储。
- `v2board-lifecycle inspect --manifest <path>`：旧系统仍运行时执行在线只读检查。
- `v2board-lifecycle inspect-release-archive ...`：以与正式 admission 相同的 inode、tar tree、内部
  `SHA256SUMS`、前端链接和 systemd 契约只读检查 native archive；不生成迁移授权。
- `v2board-lifecycle authorize ...`：能力开放后会重新执行并精确匹配已复查的稳定
  `review_binding_sha256`，只有操作者输入完整 `operation_id` 后才写 owner-only authorization；当前关闭的
  capability 会在检查后、显示确认提示前拒绝。authorization 另存当次完整动态 report digest，但它不
  fence、不停服、不改 datastore。
- `v2board-lifecycle apply ...`：只接受清单中固定路径的 authorization，启动一次不可逆离线迁移。
- `v2board-lifecycle resume ...`：只恢复同一个 durable operation，不构成第二次迁移决定。

`apply/resume` 的生产执行器已经接线，但在完整裸机故障注入门禁通过前仍由单一 typed production
capability 失败关闭；validate、inspect 的 verdict/next action、authorize、apply 与 resume 都从同一个
能力值派生，当前统一输出 `apply_available=false`，因此不能获得迁移、写入或切流许可。manifest 严格区分
[全新安装](docs/examples/fresh-install.v3.example.json)、
[旧版迁移](docs/examples/legacy-migration.v5.example.json)和
[native 升级](docs/examples/native-upgrade.v3.example.json)。当前旧站含节点时使用 schema v5：它明确放弃旧节点、
路由、历史流量明细以及旧请求/邮件发送日志，迁移后手工重建节点；schema v4 只保留为空节点来源的冻结兼容语义，
并继续迁移这两张日志表。v2 会在
字段解释前被拒绝。解除总开关必须以真实 PostgreSQL 18、ClickHouse 26.3、Oracle MySQL 8、Redis、systemd
和每个 crash/lost-ACK 边界的端到端结果为依据，不能因为某个单元阶段已实现就提前启用。

旧版迁移固定把 source Oracle MySQL 8 和旧 Redis 保持只读，先完成 inventory、fence、
pending traffic/queue drain、一致备份与隔离恢复证明，再转换到全新的 PostgreSQL 18 target。
旧 session 不转换，迁移后全量登出。schema v5 保留 `v2_stat` 每日业务汇总以及
`v2_user.u/d/transfer_enable` 等权威状态，放弃 `v2_stat_user`、`v2_stat_server` 历史流量明细和
`v2_log`、`v2_mail_log` 瞬态历史；四张 source 表仍进入完整指纹与加密归档，pre-authority PostgreSQL
copy receipt 必须以逐表 discard proof 证明旧行未进入 target。`v2_payment` 的 ID、UUID、验签配置和 `enable`
原值迁移，绝不因切换默认禁用；fence 后尚未
计入 SQL 的 Redis 流量仍 exactly-once 合入用户 `u/d`，绝不丢失当前额度。ClickHouse 从空的 native event
epoch 开始，只接收迁移后经过验证的 PostgreSQL outbox 投影，也不能用 ClickHouse 行数证明事务迁移成功。

这是一次性离线迁移，不是长期共存方案：不实现 CDC、MySQL→PostgreSQL 复制、影子读、双写、分批放量
或旧库运行时回退。真正的数据变更只能在一个维护窗口内执行一次；最终验收通过后统一启动新版并永久
mask 并停用本机 MySQL 8 和旧 Redis，使用旧凭据主动证明访问已永久不可达。旧库只允许留下经过
解密恢复和精确指纹验证、带校验和的加密备份，不得重新
接入生产。completion 成功后命令结果会输出 root 人工清理 argv，由操作者删除一次性 lifecycle 工具；
一次性 MySQL client 与 source credential 另行清理。工具是否已删除不影响已经落盘的迁移成功证明；
加密 archive、operation journal 和签名 report receipts 必须永久保留，不能随工具一起删除。

## 单一人工清单、双启动配置、单一动态权威源

操作者只手工维护一个 lifecycle JSON manifest（当前旧版迁移使用 schema v5）；旧 `.env`、Laravel 配置和 theme 脚本都不自动
导入。清单中的行为配置只在 lifecycle 进程内分别经过 API/Worker 完整 typed 解析，并证明规范化结果完全
一致；长期落盘的只有各角色最小 boot 字段与 datastore 凭据，形成两个严格的启动文档：
`/var/lib/v2board/api/config.json` 和 `/var/lib/v2board/worker/config.json`。生产 executor 已实现按角色
分目录、owner/mode 校验、no-clobber 原子安装和回读验证；总 apply 门禁关闭时不会执行这些写入。

这两个 `0600` 文件只负责角色隔离的数据库、Redis、ClickHouse、`APP_KEY`、监听与网络策略等启动材料，
不含任何动态 operator 字段或其四个敏感设置。lifecycle 在 installation 行建立后、启动服务前，使用
migration principal 和 manifest 中的 `APP_KEY` 把同一个规范化 candidate 直接加密写为 PostgreSQL 首个
active revision，不生成 API-only seed 文件。崩溃续跑只接受解密、HMAC 验证后与 candidate 完全相同的
已有 snapshot；任何错值、孤儿 revision 或 state 漂移都阻断。API/Worker 在 active revision 存在并完成
解密、typed 校验和各自 applied 回执前不进入 ready。此后管理后台的动态
系统配置只提交到 PostgreSQL 的不可变、单调 revision，API 和 Worker 都读取同一个 active pointer，不再
分别改写两个角色文件。

公开设置与四个敏感设置分开存放；`server_token`、SMTP password、Telegram bot token 和 reCAPTCHA key
使用由 `APP_KEY` 派生的 AES-256-GCM key 加密，公开 JSON 不含这些 key。保存先在内存中构造完整 typed
candidate 并校验，revision insert 与 active pointer CAS 在同一事务提交；失败不会改变 active revision。
API/Worker 各写自己的 applied/rejected 回执，且数据库权限不允许一方伪造另一方回执。
native authority 已提交且 API/Worker 都 ready 后，forward-only 启动阶段会安全删除本 operation 的
`.previous`、`.absent`、`.tmp` role-config artifact、fsync 两个父目录，并将清理证明写入 stage digest；
成功态不会遗留可能含旧完整 plaintext 配置的回滚文件。

两个生产文档都使用：

```json
{
  "configuration_source": "file_only",
  "configuration_scope": "boot_only"
}
```

上面只是启动配置的模式标记，**不是可直接启动的完整配置**。`file_only` 要求精确 key 集合、正确
`runtime_role` 和完整 boot 字段；缺 key、未知 key、错类型、非法额外 secret、错配 role 或 malformed JSON
都会失败关闭，值型环境变量也不能覆盖文件。API 文档只含 API PostgreSQL URL、worker 的非秘密
principal 名和 Redis；worker 文档只含 worker PostgreSQL URL、API 的非秘密 principal 名、Redis 与
ClickHouse writer。API 不接收任何 ClickHouse 凭据，worker 不接收 API URL 或 ClickHouse reader。
数据库 active operator map 的字段优先于值型环境变量，因此后台保存 `APP_URL` 等动态项不会出现“保存
成功但仍被环境变量覆盖”的假成功；基础设施字段不属于 operator map，也不能由后台修改。

bootstrap/schema/migration 一次性凭据、runtime path、systemd watchdog 路径和连接池大小等进程编排参数
不进入角色 JSON，继续由固定 systemd unit、credential 或部署平台注入，且不会从旧系统迁移。

两个文件都包含 secret，必须分别由对应 Unix 用户拥有，并以原子替换方式更新：

```bash
chown v2board-api:v2board-api /var/lib/v2board/api/config.json
chmod 0700 /var/lib/v2board/api
chmod 0600 /var/lib/v2board/api/config.json
chown v2board-worker:v2board-worker /var/lib/v2board/worker/config.json
chmod 0700 /var/lib/v2board/worker
chmod 0600 /var/lib/v2board/worker/config.json
```

文件拆分还不等于权限隔离：生产 API 与 Worker 使用不同用户和不同可写父目录，不能共享 config 目录；
否则 API 的原子 rename 权限可能影响 Worker 文件。`V2BOARD_CONFIG_PATH` 只能由 systemd unit 固定为本
进程路径，不能绕过 role 与额外 secret key 校验。

进程启动时配置无效会拒绝启动；运行中的错误编辑会保留最后一个已验证快照并记录错误。datastore
URL/凭据、监听地址、route、连接/请求 client 或 KDF 并发等已被进程捕获的字段即使语法正确也不会
伪装成热切换，修改后必须重启。不要提交填写后的配置。数据库 URL 中的特殊用户名或密码必须
percent-encode。

### 数据存储权限分离

- API 和 worker 各自在自己的文档中使用 `database_url`；`peer_database_principal` 只保存对方的非秘密
  role 名，用于拒绝 principal 复用。migration job 使用第三个、只在迁移窗口存在的 DDL principal，
  并同时与 API/worker role 不同。
- ClickHouse schema principal 只注入一次性 schema job。当前 API 不查询 ClickHouse，因此不持有
  reader 或 writer；worker relay 只持有 raw/按批次日聚合 INSERT、核对自身批次和 readiness 所需的
  受限 SELECT writer，
  不持有 reader 或 DDL 凭据。
- 生产 PostgreSQL URL 必须使用 `sslmode=verify-full`，ClickHouse 必须使用经过证书验证的
  `https://` origin，Redis 必须使用 `rediss://`。本地 Compose 的明文连接只允许存在于隔离开发网络。

## 本地 Docker 开发

所有依赖安装、构建、migration 和测试都在 Docker 中完成：

```bash
make up
make sync        # 修改源码后重建/刷新 Docker workspace 与部署 release
make doctor
```

`docker-compose.local.yml` 固定使用 PostgreSQL 18.4、ClickHouse 26.3 LTS、Redis 8.8，并包含两个独立的
一次性 migration service：

- `rust-migrate`：运行 `v2board-api migrate`，只管理 PostgreSQL schema lineage。
- `clickhouse-migrate`：运行 `v2board-analytics-schema`；本地在 PostgreSQL seed 完成后绑定同一 installation
  并应用 90/730 天 raw/aggregate TTL。生产不使用 Compose，由 lifecycle schema principal 执行清单值。

本地端口如下；数据库端口暴露只为开发诊断，生产不应直接公开：

| 服务 | 地址/端口 |
|---|---|
| Rust 应用与 API | <http://localhost:8000> |
| 用户端 Vite | <http://localhost:5173> |
| 管理端 Vite | <http://localhost:5174/admin> |
| PostgreSQL | `localhost:5432` |
| ClickHouse HTTP / native | `localhost:8123` / `localhost:9000` |
| Redis | `localhost:6379` |
| Mailpit SMTP / UI | `localhost:1025` / <http://localhost:8025> |
| 只读 reference oracle（可选） | <http://localhost:8001> |

本地 PostgreSQL schema 由 Rust migration 创建；ClickHouse schema、installation binding 与 retention 由
依赖 PostgreSQL migration 成功的独立 migration service 创建。
`V2BOARD_SEED_LOCAL=1` 默认生成管理员 `admin@example.com` / `12345678` 和最小测试数据，生产环境
禁止开启。

常用验证命令：

```bash
make doctor                 # Compose、宿主输出、runtime 隔离和配置审计
make rust-check             # fmt + clippy
make rust-test              # workspace tests
make rust-integration       # PostgreSQL/ClickHouse/Redis 与 analytics outbox 往返
make native-database-audit  # 新版 runtime 禁止旧数据库 driver/方言回流
make rust-target-gate       # Rust 全量发布门禁
make deploy-smoke
make behavior-parity
```

`make reset` 会删除 PostgreSQL、ClickHouse、Redis、Rust runtime、依赖和构建 volumes 后重建，属于
破坏性本地操作。需要保留数据时只使用 `make sync`；不要寻找或调用已经移除的旧数据库运行时
升级 helper。

## 裸机生产发布与启动顺序

Docker 只用于本地开发、CI、测试和可复现构建，生产服务器不安装 Docker。CI 从
`Dockerfile.rust` 的 `native-release` export stage 生成原生 Linux release，其中只包含：

- `v2board-api`、`v2board-workers`、`v2board-analytics-schema` 三个 ELF；
- 已验证的 immutable frontend release；
- `v2board-api.service`、`v2board-worker.service`；
- `RELEASE` 和 `SHA256SUMS`。

当前发布目标固定 Debian 12 compatible Linux amd64/glibc。服务器先验证归档 SHA-256，再解压到
root-owned `/opt/v2board/releases/<release-id>`，最后原子更新 `/opt/v2board/current`；不得在服务器编译。
生产至少准备 PostgreSQL 18.x、ClickHouse 26.3 LTS 和 Redis，它们由各自原生 system service 或托管
平台维护，不由 V2Board unit 安装或启动。

长期进程固定使用两个无登录 Unix 用户和完全分离的可写目录：

- `v2board-api` 只写 `/var/lib/v2board/api`，读取 `/var/lib/v2board/api/config.json`；
- `v2board-worker` 只写 `/var/lib/v2board/worker`，读取 `/var/lib/v2board/worker/config.json`；
- `/var/lib/v2board/rules` 与 `/opt/v2board/current` 由 root 拥有、进程只读；
- 两份 config 均为各自 owner 的 `0600`，父目录为各自 owner 的 `0700`，不能使用共享可写 config 目录。

完整安装边界见[裸机部署指南](deploy/README.md)，systemd unit 位于 [`deploy/systemd`](deploy/systemd)。
API 只绑定 `127.0.0.1:8080`，由 Nginx/Caddy
终止 TLS 并把页面、API 和 `/assets/*` 全部反代给 Rust。Worker 使用 `Type=notify` 和 `WatchdogSec=30s`；
只有 PostgreSQL、精确 migration ledger 与 Redis 探测成功才发送 `READY=1`，持续探测失败时 systemd
会重启它。API/worker 在监听或启动任务前也会拒绝非 exact-current PostgreSQL schema。

一次性 `v2board-lifecycle` 是独立 migration tool，只有它允许携带只读 MySQL source adapter。它不进入
长期 native release；旧 source 全部 unit 已永久 mask、旧凭据实测不可达且永久 PostgreSQL ledger
提交后，才可从服务器删除该工具。普通
`v2board-api migrate` 也不是 legacy converter：生产中它只核验已 active 且与当前 binary 完全一致的
PostgreSQL ledger，fresh/legacy/forward-upgrade DDL 必须等待 lifecycle apply。

release 启用顺序固定为：验证 release/配置/数据库备份 → 串行 schema lifecycle job → 原子更新
`/opt/v2board/current` → 启动 API → `/readyz` 通过 → 启动 Worker。API 与 Worker 的 stdout/stderr 进入
journald，必须配置磁盘上限和 retention；PostgreSQL、ClickHouse、Redis 使用各自原生日志轮转。

### 健康与 ClickHouse 故障语义

- API `/readyz` 检查 PostgreSQL migration、Redis 和前端 release。
- worker 的 systemd watchdog 检查 PostgreSQL exact migration 与 Redis；worker 不开放 HTTP 端口。
- API 当前不查询 ClickHouse，短暂中断不进入 API readiness；订单、认证、支付和 PostgreSQL 流量结算
  可继续，analytics 标记 stale/unavailable，outbox 保留并重试。
- 这不是“任意长故障都不影响核心”的承诺。PostgreSQL 内已有 installation-bound 的
  normal/soft/hard outbox admission：精确采样 pending rows、relation/database bytes、oldest age 和磁盘
  headroom；soft 对新增分析事件限速，hard 或 stale sample 只拒绝相关流量事务，relay 仍持续排空。
- ClickHouse 当前策略明确为单节点、可从 PostgreSQL outbox 重建的投影；它已有串行可恢复 schema
  migration、精确 lineage/installation 绑定和 manifest-bound TTL。Keeper/ReplicatedMergeTree 属于提高
  可用性的部署选项，不伪装成事务正确性的必需条件；PostgreSQL 的备份/PITR 与 outbox 才是权威恢复源。
- 本地 Compose 对全部服务使用有界 `local` 日志（10MB × 3）；生产使用有界 journald 和数据库原生日志
  轮转，日志耗尽共享磁盘必须按数据库不可用处理。
- 必须告警 pending rows/bytes、oldest outbox age、publish/retry rate 和 ClickHouse merge/replica lag；
  不得把陈旧投影伪装成实时权威数据，也不得回退为对 PostgreSQL 的无界分析扫描。

管理员密码重置使用原生命令 `v2board-api reset-admin-password <email>`；生产一次性 secret 必须通过
root-only credential file/systemd credential 注入，不能保留在 unit Environment 或 shell history，且不需要 PHP 工具。

## 参考实现

参考项目固定在 git submodule 中，只允许只读使用：

```bash
git submodule update --init --recursive
make reference-oracle-check
make reference-oracle-up      # 可选：localhost:8001 手工兼容性查看
```

兼容测试直接从 `references/wyx2685-v2board` 读取旧资源，不会复制到当前源码、生产 release artifact 或 deploy
volume。行为契约以真实 API、路由、持久化键和外部集成为准，而不是旧 DOM 或像素输出。
