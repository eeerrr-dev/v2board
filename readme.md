# V2Board Native

V2Board 的现代化实现：单一 Rust 后端、Rust worker，以及 React + TypeScript +
Vite + shadcn/ui 前端。PHP/Laravel 与旧打包前端不再属于运行、构建或部署链路。

## 架构

- `backend/rust`：Axum API、后台任务、SQLx migration 与本地开发 seed。
- `frontend/apps/user`：用户端 React 应用。
- `frontend/apps/admin`：管理端 React 应用。
- `frontend/packages`：类型、API client、i18n 与共享构建配置。
- `references/wyx2685-v2board`：只读上游参考；仅用于兼容性审计，绝不参与生产构建。

Rust 在一个端口同时提供 API、用户页、动态后台路径和带内容哈希的静态资源。
Vite 的不可变 release 位于 `dist-deploy/releases/*`，`current` 原子地指向当前版本。

## 安装与升级契约

在实现或执行全新安装、从 pinned reference 旧版迁移、native 升级或破坏性升级前，必须遵守
[安装、旧版迁移与升级不可变契约](docs/upgrade-invariants.md)。未在该契约中明确归为可重建的
现有状态默认按最高保护级处理；来源不明、无法验证的半迁移或存在歧义时禁止写入。可验证的 pending
operation 只能按原计划 resume/recover，不得开始新的升级。

手填字段、双 Redis DB、全量登出、target MySQL 自动创建意图和节点维护切换的说明见
[唯一旧版迁移清单 v2](docs/legacy-migration-manifest.md)。

### 当前只读 lifecycle 工具

当前只提供三条**不会执行迁移**的命令：

- `v2board-api provision validate --manifest <path>`：校验 secret-bearing JSON 文件的权限、版本、
  duplicate/unknown/missing key、完整 file-only AppConfig、运行时同源语义和固定迁移决策；不连接数据库，
  也不写文件。
- `v2board-api provision inspect --manifest <path>`：旧系统仍运行时执行在线只读兼容检查；
  verdict 为 `compatible|blocked`，不会进入维护。
- `v2board-api provision plan --manifest <path>`：只在操作者已确认进入维护、完成 fence/drain/
  backup/restore proof 后执行最终只读检查；verdict 为 `ready_for_confirmation|blocked`。

`inspect` 和 `plan` 只连接 source MySQL/Redis、target MySQL `bootstrap_database_url` 与 target Redis，
输出脱敏 inventory；不复制数据、不排空队列、不创建 target 数据库/账号/表、不生成节点凭据，
也不写 `config.json`。MySQL 版本由服务器自动报告：source 仅接受 MySQL/Percona 5.7+；target
要求 MySQL 8.4+，且 `application_database_url` 指定的库名和解码后的应用 `'user'@'host'` 必须不存在。
bootstrap 凭据必须能从 `information_schema` / `mysql.user` 证明这两个事实。用户无需、也不应手工预建
target 库或账号；future `apply` 只能在最终确认后用 bootstrap 凭据创建
`utf8mb4/utf8mb4_unicode_ci` 库、受限应用账号和 native schema。`application_account_host`
明确指定 MySQL `'user'@'host'` 中的客户端来源范围，不是 DSN 服务器 host，只接受精确 hostname/IP
或规范 IPv4 CIDR，不接受 `%`/`_` wildcard。`require_database_absent=true` 与
`require_account_absent=true` 都是强制项。
target Redis 没有建立逻辑 DB
的操作；工具只验证选定 DB/namespace 为空，绝不 `FLUSHDB`。

唯一 legacy v2 固定为 `manual_only`（不导入旧配置）、`logout_all`（不转换旧 session）、
`discard_ephemeral_after_fence`（不复制已 fencing 的可重建 cache）和 `maintenance_cutover`
（不启用在线 global-token 双协议窗口）；Stripe 与临时订阅 token 均固定 `assert_none`，旧主题固定放弃，
自定义规则只能声明不存在或确认放弃。这些是清单约束，不代表对应 apply 动作已经实现；口头声明“没用
Stripe”不能代替 plan 的数据库零状态检查。

[v2 示例清单](docs/examples/legacy-migration.v2.example.json) 是带占位符的公开模板，权限也是仓库普通
只读文件，**不能直接运行**。先复制到仓库外的受保护 secret 路径，逐项复核并填写所有字段（不能只
替换 `REPLACE`），生成新的 operation UUID，再设为仅 owner 可读写：

```bash
cp docs/examples/legacy-migration.v2.example.json /secure/private/legacy-migration.json
chmod 600 /secure/private/legacy-migration.json
v2board-api provision validate --manifest /secure/private/legacy-migration.json
v2board-api provision inspect --manifest /secure/private/legacy-migration.json
# compatible 后，只在明确进入维护并完成 fence/drain/backup/restore proof 后执行：
v2board-api provision plan --manifest /secure/private/legacy-migration.json
```

不要提交填写后的清单。独立的 `lifecycle_audit_key` 不得与 app/node secret 或 target datastore 密码共用；它把原始清单完整 bytes
绑定进报告，任何配置、凭据、备份引用或格式变化都会使旧确认失效。数据库 URL 中的特殊用户名/密码
必须进行 URL percent-encoding，校验器会先解码再检查，编码不能绕过账号分离或 placeholder 规则；source 的明文
MySQL/Redis 连接只允许在可信私网或加密隧道中使用，target bootstrap/application 生产连接必须使用受验证的
TLS。

当前 legacy 检查只接受可探测为 standalone 的拓扑：MySQL UUID 必须有效、非零、彼此不同，且不能检测到
replication channel、group member 或 binlog replica client；Redis 必须是 master、零 connected replica、
关闭 cluster，且 source/target `run_id` 不同。未注册或离线 replica、底层 storage failure domain 仍无法靠
单节点查询完全证明，所以完整 topology binding 继续作为 `implementation_blocker`。

旧版 MySQL `v2_user.token` 是永久订阅凭据，必须原值迁移；Redis `otp_`/`otpn_` 是短期一次性
映射，`totp_` 只是已验证时间窗的临时 cache，而 TOTP URL 在签发时甚至可以不写 Redis。
Redis 中的 `v2board_upload_traffic` / `v2board_download_traffic` 则是节点已上报但尚未累加到
MySQL `u/d` 的权威增量，不是可丢 cache。完整边界见上述清单。

固定流程是：在线 `inspect` 全部通过 → 第一次确认是否进入维护 → fence/drain/backup/restore proof
→ 最终 `plan` 全部通过并展示脱敏摘要 → 第二次确认精确 `operation_id + report_sha256` → future
`apply`。仓库当前没有 `provision apply`，实现边界明确为 `apply_available=false`；`compatible` 和
`ready_for_confirmation` 都不是切流或迁移许可。`report_sha256` 是将其自身置空后的 canonical
report payload 摘要，且包含 `manifest_binding_hmac_sha256` 与现场实例 identity，并非最终打印 JSON
文件的 `sha256sum`。v2 `runtime`
仅覆盖 file-only AppConfig keys；database pool、worker lifecycle/retention 和 runtime/rules/frontend path
bootstrap 尚未纳入 materialize/promote。少量旧配置事实和未分类 Redis key 也尚未机器证明，因此仍是
apply blocker。它们会明确列在 `implementation_blockers`；当前在线命令失败关闭为
`blocked/resolve_blockers`。只有这些实现缺口全部关闭且 apply 真正可用后，才会返回
`compatible/confirm_enter_maintenance`。

## 本地开发

所有依赖安装、构建和测试都在 Docker 内完成：

```bash
make up
make sync        # 修改源码后刷新 Docker workspace
make doctor
```

本地入口：

- 应用/API：<http://localhost:8000>
- 用户端 Vite：<http://localhost:5173>
- 管理端 Vite：<http://localhost:5174/admin>
- Mailpit：<http://localhost:8025>

本地数据库由 Rust migration 创建。`V2BOARD_SEED_LOCAL=1` 默认生成管理员
`admin@example.com` / `12345678` 和最小测试数据；生产环境不要开启此变量。

常用检查：

```bash
make rust-check
make rust-test
make deploy-smoke
make visual-smoke
make behavior-parity
```

`make reset` 会删除 MySQL、Redis、Rust runtime、依赖和构建 volumes；需要保留数据时
只使用 `make sync`。

从旧的 MySQL 8.0 本地 volume 首次升级到 8.4 时，先运行
`make mysql-auth-upgrade`。它只在隔离的一次性容器中临时加载旧认证插件，把本地
`root`/`v2board` 账户迁移到 `caching_sha2_password`，随后立即以禁用旧插件的正常
8.4 服务重启；不会删除数据库。生产数据库请按 [backend/README.md](backend/README.md)
中的维护窗口步骤迁移并轮换真实密码。

## 生产镜像

根目录的 `Dockerfile.rust` 是生产构建入口。`production-api` 与
`production-worker` 是独立的生产 stage；默认 `production` 兼容别名指向 API。
它们会：

1. 以锁文件构建 release API 与 worker；
2. 仅为 API 构建并验证 user/admin 的 manifest-driven Vite release；
3. 分别生成最小、非 root、使用各自健康检查与日志默认值的运行镜像。

下面的 `migrate` 是 native SQLx schema runner，只能用于已确认的空库或 native lineage，并不构成成熟的
fresh-install workflow。**绝不能把 `DATABASE_URL` 指向 reference 旧库，也不能用它代替旧版迁移器。**

```bash
docker build --target production-api -f Dockerfile.rust -t v2board-api .
docker build --target production-worker -f Dockerfile.rust -t v2board-worker .
docker run --rm \
  -e DATABASE_URL='mysql://user:pass@db.example.com:3306/v2board?ssl-mode=VERIFY_IDENTITY' \
  -e REDIS_URL='rediss://cache.example.com:6380/1' \
  -e APP_URL='https://app.example.com' \
  -e APP_KEY='<inject-at-least-32-random-bytes>' \
  -e V2BOARD_SERVER_TOKEN='<inject-a-different-32-byte-random-secret>' \
  -e V2BOARD_TRUSTED_PROXY_CIDRS='10.42.0.10/32' \
  -v v2board-runtime:/var/lib/v2board \
  v2board-api /usr/local/bin/v2board-api migrate
docker run --rm -p 8000:8080 \
  -e DATABASE_URL='mysql://user:pass@db.example.com:3306/v2board?ssl-mode=VERIFY_IDENTITY' \
  -e REDIS_URL='rediss://cache.example.com:6380/1' \
  -e APP_URL='https://app.example.com' \
  -e APP_KEY='<inject-at-least-32-random-bytes>' \
  -e V2BOARD_SERVER_TOKEN='<inject-a-different-32-byte-random-secret>' \
  -e V2BOARD_TRUSTED_PROXY_CIDRS='10.42.0.10/32' \
  -v v2board-runtime:/var/lib/v2board \
  v2board-api
docker run --rm \
  -e DATABASE_URL='mysql://user:pass@db.example.com:3306/v2board?ssl-mode=VERIFY_IDENTITY' \
  -e REDIS_URL='rediss://cache.example.com:6380/1' \
  -e APP_URL='https://app.example.com' \
  -e APP_KEY='<inject-at-least-32-random-bytes>' \
  -e V2BOARD_SERVER_TOKEN='<inject-a-different-32-byte-random-secret>' \
  -e V2BOARD_TRUSTED_PROXY_CIDRS='10.42.0.10/32' \
  -v v2board-runtime:/var/lib/v2board \
  v2board-worker
```

尖括号值只是 secret-manager 占位符，不是可复用密钥，而且生产校验会明确拒绝这些占位符；
运行前必须由部署密钥存储替换。生产模式会拒绝明文数据存储连接：
MySQL URL 必须包含 `ssl-mode=VERIFY_IDENTITY`（私有 CA 另加 `ssl-ca`），Redis 必须使用
`rediss://`；本地 Compose 的明文连接只用于隔离开发网络。

唯一旧版迁移固定使用维护切换，不开启在线 global-token 兼容窗口：先停止旧 API/worker/scheduler 和
所有 node reporter，排空 queue 并结清旧 Redis traffic hash；转换完成后保持
`V2BOARD_SERVER_LEGACY_TOKEN_ENABLE=false` 和 `V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY=true`，
从管理端 `server/manage/getNodes` 取得各节点独立的 `n1_...` 凭据，手工更新 endpoint/token，并让每个
流量批次携带稳定且不可复用于其他 payload 的 `Idempotency-Key`。逐节点验证 config/users 拉取和
traffic 幂等计费后，才恢复 reporter。当前只读 preflight 只会报告节点数量和这一维护要求，不会执行
上述操作。

先把 migration 命令作为平台串行的一次性 Job 运行成功，再启动 API 与 worker。API
healthcheck 使用 `/readyz`，schema checksum、MySQL、Redis 或前端 release 未就绪时
不会把实例标为健康。worker 不开放 HTTP 端口；它使用经过 MySQL/Redis 探测后更新的
本地时间戳健康文件，任一调度循环异常退出时整个进程失败退出并由编排器重启。生产数据只写入
`/var/lib/v2board`；镜像内前端 release 是只读的。反向代理应把所有应用流量交给
Rust，并为 `/assets/user/*` 与 `/assets/admin/*` 保留其 immutable cache header。若
使用 bind mount/PVC，需确保 `/var/lib/v2board` 对镜像内 UID/GID `10001:10001` 可写。

管理员密码重置使用原生命令：为容器注入一次性的 `V2BOARD_NEW_PASSWORD`，执行
`v2board-api reset-admin-password <email>`；开发镜像中可等价执行
`cargo run -p v2board-api -- reset-admin-password <email>`，无需任何 PHP 工具。

## 参考实现

参考项目固定在 git submodule 中，只允许只读使用：

```bash
git submodule update --init --recursive
make reference-oracle-check
make reference-oracle-up      # 可选：localhost:8001 手工兼容性查看
```

兼容测试直接从 `references/wyx2685-v2board` 读取旧资源，不会复制到当前源码、生产镜像
或 deploy volume。行为契约以真实 API、路由、持久化键和外部集成为准，而不是旧 DOM
或像素输出。
