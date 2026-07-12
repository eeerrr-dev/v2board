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

MySQL、Percona 和 MariaDB 都不是新版可选运行时。它们只可能作为
`references/wyx2685-v2board` 所代表旧系统的**只读 source**，经明确识别后进入维护式迁移。
完整边界见 [PostgreSQL + ClickHouse 持久化不变量](docs/postgresql-clickhouse-invariants.md)。

Rust 在一个端口同时提供 API、用户页、动态后台路径和带内容哈希的静态资源。前端部署根包含
不可变 `releases/<content-id>/{user,admin}`，并通过 `current`/`previous` 原子链接切换版本。

## 安装、旧版迁移与升级

全新安装、旧版迁移、native 升级和破坏性升级都必须遵守
[安装、旧版迁移与升级不可变契约](docs/upgrade-invariants.md)以及
[PostgreSQL + ClickHouse 持久化不变量](docs/postgresql-clickhouse-invariants.md)。来源不明、
无法验证的半迁移或 installation identity 不一致时必须停止，不能猜测后继续写入。

当前一次性 lifecycle CLI 只有三条只读命令：

- `v2board-lifecycle validate --manifest <path>`：严格校验 lifecycle JSON，不连接数据存储。
- `v2board-lifecycle inspect --manifest <path>`：旧系统仍运行时执行在线只读检查。
- `v2board-lifecycle plan --manifest <path>`：进入 fenced maintenance 后执行最终只读计划检查。

当前**没有** `apply`。检查报告继续以 `apply_available=false` 和实现 blocker
失败关闭，不能获得迁移、写入或切流许可。manifest v3 已冻结并严格区分
[全新安装](docs/examples/fresh-install.v3.example.json)、
[旧版迁移](docs/examples/legacy-migration.v3.example.json)和
[native 升级](docs/examples/native-upgrade.v3.example.json)；v2 会在字段解释前被拒绝。自动化仍必须等待
converter、journal、target bootstrap、one-shot final-recheck/apply 执行器和全部门禁完成。

旧版迁移固定把 source MySQL/MariaDB 和旧 Redis 保持只读，先完成 inventory、fence、
pending traffic/queue drain、一致备份与隔离恢复证明，再转换到全新的 PostgreSQL 18 target。
旧 session 不转换，迁移后全量登出；ClickHouse 只能从经过验证的 PostgreSQL outbox 或明确标注的
legacy aggregate 重建，不能用 ClickHouse 行数证明事务迁移成功。

这是一次性离线迁移，不是长期共存方案：不实现 CDC、MySQL→PostgreSQL 复制、影子读、双写、分批放量
或旧库运行时回退。真正的数据变更只能在一个维护窗口内执行一次；最终验收通过后统一启动新版并永久
停用 MySQL/MariaDB 和旧 Redis，撤销账号与网络访问。旧库只允许留下带校验和的加密冷归档，不得重新
接入生产。迁移后服务器也不保留一次性 lifecycle/MySQL source 工具。

## 单一人工清单、双运行时配置

操作者只手工维护一个 lifecycle v3 JSON manifest；旧 `.env`、Laravel 配置和 theme 脚本都不自动
导入。清单中的公共行为配置会分别与 API/worker 所需的最小 datastore 凭据组合，形成两个严格文档：
`/var/lib/v2board/api/config.json` 和 `/var/lib/v2board/worker/config.json`。当前 CLI 只在内存中构造并
验证这两个 map，尚不写文件或执行 apply。

两个生产文档都使用：

```json
{
  "configuration_source": "file_only"
}
```

上面只是模式标记，**不是可直接启动的完整配置**。`file_only` 要求精确 key 集合、正确
`runtime_role` 和完整公共字段；缺 key、未知 key、错类型、非法 secret、错配 role 或 malformed JSON
都会失败关闭，值型环境变量也不能覆盖文件。API 文档只含 API PostgreSQL URL、worker 的非秘密
principal 名和 Redis；worker 文档只含 worker PostgreSQL URL、API 的非秘密 principal 名、Redis 与
ClickHouse writer。API 不接收任何 ClickHouse 凭据，worker 不接收 API URL 或 ClickHouse reader。

bootstrap/schema/migration 一次性凭据、systemd watchdog 路径和连接池大小等进程编排参数不属于长期
应用配置，继续由 systemd credential/部署平台注入，且不会从旧系统迁移。

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
  reader 或 writer；worker relay 只持有 raw INSERT 和核对自身批次所需受限 SELECT 的 writer，
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
- `clickhouse-migrate`：运行 `v2board-analytics-schema`，只管理 ClickHouse schema lineage。

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

本地 PostgreSQL schema 由 Rust migration 创建；ClickHouse schema 由独立 migration service 创建。
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
长期 native release；迁移成功后必须从服务器删除并撤销全部 source credential。普通
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
- 这不是“任意长故障都不影响核心”的承诺。outbox 容量预算、磁盘水位与安全背压门禁尚未实现；在其
  完成前，长期 ClickHouse 故障是生产 blocker，也是三类 lifecycle `apply=false` 的原因之一。
- ClickHouse 固定单节点已有串行且可崩溃恢复的 schema migration、精确 lineage 校验和 installation
  binding；但 TTL/archive/restore drill、HA/ReplicatedMergeTree/Keeper 以及跨节点 schema 协调尚未完成。
  双文件 secret split 完成后仍不能解除 fresh、legacy 或 native upgrade 的 apply blocker。
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
