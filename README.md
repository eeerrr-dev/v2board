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
  migration ledger、installation identity 和待发布 analytics outbox 都以它为准。
- **ClickHouse 26.3 LTS**：只保存 PostgreSQL 先提交的不可变分析投影，绝不反写或决定核心业务状态。
- **Redis**：只保存 session、限流、lease、锁和短期缓存，不保存不可恢复账本。

MySQL 不是新版可选运行时。唯一允许的旧版数据库来源是
`references/wyx2685-v2board` 所代表、且精确识别为 Oracle MySQL 8.0/8.4 的**只读 source**；
MySQL 5.7、Percona 和 MariaDB 均不在迁移支持面内。当前 converter 在导入期间需要一个临时
MySQL 8 engine 来读取 dump，但新版生产运行时不安装、不保留 MySQL 服务。
完整边界见 [PostgreSQL + ClickHouse 持久化不变量](docs/postgresql-clickhouse-invariants.md)。

Rust 在一个端口同时提供 API、用户页、动态后台路径和带内容哈希的静态资源。前端部署根包含
不可变 `releases/<content-id>/{user,admin}`，并通过 `current`/`previous` 原子链接切换版本。

## MySQL 一次性导入

native 新版尚未发布，也没有已经安装的 native 数据库需要兼容。首发前只保留一个
[`mysql-import.v1`](docs/examples/mysql-import.v1.example.json) 流程：

1. 停止旧站全部写入；
2. 导出完整 Oracle MySQL 8 dump；
3. 把 dump 导入一次性 MySQL 8 engine；
4. 转换保留数据到全新的 PostgreSQL 18；
5. ClickHouse 26.3 和新 Redis 从空状态开始；
6. 从清单生成新的 API/worker 配置；
7. 验证后启动新版。

旧 MySQL 不被修改，staging 只用于处理 dump。它可以是迁移机/新机上的一次性容器、独立临时 VM，
或其他受控临时主机；导入成功或失败后删除，不是新服务器的长期依赖。导入器不连接旧 Redis，也不
联系 Stripe provider。旧 MySQL source 表保留真实 `v2_*` 名；新 PostgreSQL/ClickHouse target 表不带
该前缀，其中关键字冲突使用 `users` 和 `orders`。
旧 Redis 全部状态、Stripe 配置和未完成 Stripe 订单固定丢弃；terminal Stripe 订单只保留解绑
provider 的业务历史，非 Stripe 业务数据和用户余额按固定规则保留。

当前 MySQL 导入命令只有：

- `v2board-lifecycle validate --manifest <path>`：严格校验 `mysql-import.v1`；
- `v2board-lifecycle inspect --manifest <path>`：只读检查 dump 及静态 target/runtime 声明。

生产 converter executor 尚未接线，当前没有写入命令，不能宣称已经可以执行生产导入。

导入没有在线迁移、CDC、双写、渐进切流或中间状态续跑。失败时旧 MySQL 仍保持原状；丢弃
不完整的新 target，修正问题，再从 dump 向新的空 target 运行一次即可。完整步骤见
[MySQL 一次性导入指南](docs/mysql-import.md)，固定数据边界见
[MySQL 一次性导入不可变契约](docs/mysql-import-invariants.md)。

## 单一导入清单、双启动配置、单一动态权威源

操作者只维护一个 `mysql-import.v1` JSON，内容为 `source`、`target` 和 `runtime`。旧 `.env`、
Laravel 配置、theme 和 operator script 都不导入。`runtime` 分别经过 API/worker 完整 typed
解析，并证明规范化结果一致；长期落盘的只有各角色最小 boot 字段与 datastore credential：

- `/var/lib/v2board/api/config.json`
- `/var/lib/v2board/worker/config.json`

两个 `0600` 文件只携带各自角色所需的 PostgreSQL、Redis、ClickHouse、`APP_KEY`、监听和网络策略
材料，不包含动态 operator 字段。动态设置写入 PostgreSQL 的不可变 revision，API 和 worker 读取
同一个 active pointer 并分别记录 applied/rejected 结果。`server_token`、SMTP password、Telegram bot
token 和 reCAPTCHA key 使用由 `APP_KEY` 派生的 AES-256-GCM key 加密，公开 JSON 不包含这些 secret。

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
  并应用 90/730 天 raw/aggregate TTL。生产不使用 Compose，由 ClickHouse schema principal 执行清单值。

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
破坏性本地操作。需要保留数据时只使用 `make sync`。

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
平台维护，不由 V2Board unit 安装或启动。生产长期服务清单不包含 MySQL；若在新机上用一次性容器
完成 staging，验收后即删除该容器及其临时 volume。

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

一次性 `v2board-lifecycle` 是独立 MySQL import tool，不进入长期 native release。当前命令只检查
清单和 dump；未来 executor 才读取一次性 staging MySQL engine，并且不会连接旧 MySQL、旧 Redis 或
Stripe。该 engine 可位于独立迁移主机，不要求在旧服务器或新生产服务器永久安装 MySQL。
验收后由操作者删除该工具。普通 `v2board-api migrate` 是 PostgreSQL schema runner，不是 MySQL
converter；首次 PostgreSQL baseline 由 importer 在空 target 上执行。

release 启用顺序固定为：验证 release/配置/数据库备份 → 串行 schema job → 原子更新
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
