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

旧节点升级时，应短期开启 `V2BOARD_SERVER_LEGACY_TOKEN_ENABLE=true`，从管理端
`server/manage/getNodes` 取得各节点独立的 `n1_...` 凭据，逐节点替换全局 token，并让每个
流量批次携带稳定且不可复用于其他 payload 的 `Idempotency-Key`。验证所有 reporter 后关闭
legacy 开关并保持 `V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY=true`。管理端保存节点时传
`rotate_credential=1` 会立即只吊销该节点旧凭据，不影响其他节点。

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
