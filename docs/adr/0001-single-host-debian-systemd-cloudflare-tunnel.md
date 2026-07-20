# 0001. 单机 Debian 13 + systemd 直跑原生二进制,Cloudflare Tunnel 为唯一公网入口

- 状态:已采纳(回填)
- 日期:2026-07(回填;决策早于此日期落地)

## 背景(Context)

这是一个单操作者维护的订阅面板,生产规模是一台服务器。旧站是
PHP/Laravel + Nginx 的传统栈,证书、公网监听、反向代理配置都要人工维护。
新版的约束:

- 运维人力极少,每多一个生产组件就多一个长期维护面;
- Docker 已经承担了本地开发、CI、测试和可复现构建,但没有理由把容器运行时
  也搬上生产;
- 公网面需要 TLS、CDN、WAF 和 DDoS 防护,而这些自己做的成本远高于交给边缘
  服务;
- 主机希望做到"没有任何公网监听端口",把入站攻击面压到零。

## 决策(Decision)

生产只支持 Debian 13(Trixie)amd64:CI 从 `Dockerfile.rust` 的
`native-release` stage 导出 `v2board-api`、`v2board-workers`、
`v2board-analytics-schema` 三个 ELF,连同已验证前端树和三个 systemd unit 打包;
服务器验证 SHA-256 后解压到不可变 `/opt/v2board/releases/<release-id>`,原子
切换 `/opt/v2board/current`,由 systemd 直接运行,绝不在服务器编译。

唯一公网入口是同机 systemd 运行的 remotely-managed named Cloudflare Tunnel:
`cloudflared`(官方 stable apt 包)出站连接 Cloudflare,把唯一 public hostname
转发到 `http://127.0.0.1:8080`;connector token 是 root-only systemd credential,
unit 用 route-free `{}` `SetCredential` 关闭本地配置发现。主机防火墙拒绝入站
80/443/8080,运行时 `trusted_proxy_cidrs` 固定 `["127.0.0.1/32"]`,Rust 只从
该本机对端接受单值 `CF-Connecting-IP`。

**明确拒绝的替代方案:**

- **Nginx 或任何第二 ingress/反向代理** —— 双份 TLS/头处理/缓存语义,信任模型
  被稀释;Rust 已拥有 HTML、静态资源、压缩、缓存、CORS 与安全响应头。
- **生产容器化(production Compose/镜像部署)** —— Docker 只是本地/CI 边界;
  生产多一层容器运行时没有换来任何隔离收益(API/worker 已用独立 Unix 用户)。
- **HA 集群/多机部署** —— 单机是明确的产品规模决策;Keeper/副本属于未来可用性
  扩展,不伪装成现在的正确性需求。
- **operator 自管的本地 tunnel YAML/第二种 Cloudflare 连接模式** —— 路由权威
  只在 Cloudflare 侧一处。

## 后果(Consequences)

- 得到:主机零公网监听端口;公网 TLS/CDN/WAF/DDoS 全部由 Cloudflare 承担;
  部署物是一个签名归档 + 原子软链,回滚是一条 `mv -T`;单一 ingress 让
  `CF-Connecting-IP` 信任模型可以写死成一行 CIDR。
- 代价:**单点**。可用性上限就是这台机器加 Cloudflare;没有水平扩展路径,
  故障恢复依赖备份/PITR 而不是切换副本。
- 代价:Cloudflare 成为强依赖 —— 必须遵守一整套边缘纪律(不 Cache Everything、
  不对外部命名空间发交互挑战、不启用 Pseudo IPv4、Logpush 只选 path 字段),
  这些都写进了部署文档并有 `make cloudflared-config-audit` 看守。
- 代价:激活顺序有仪式感(先 API `/readyz`,再 worker `READY=1`,最后才开
  Tunnel),操作错误的后果由文档而非自动化兜底。

## 证据

- [`../../AGENTS.md`](../../AGENTS.md) — Source And Deploy Rules(生产边界与
  Cloudflare 纪律的规范来源)。
- [`../../deploy/README.md`](../../deploy/README.md) — 安装、Tunnel 配置与激活
  顺序。
- [`../../deploy/systemd/v2board-cloudflared.service`](../../deploy/systemd/v2board-cloudflared.service)
  与同目录另外两个 unit。
- [`../../backend/rust/crates/api/src/runtime/ingress.rs`](../../backend/rust/crates/api/src/runtime/ingress.rs)
  — `CF-Connecting-IP` 信任实现与 loopback 探测门。
- [`../operations.md`](../operations.md) — 单机日常运维与回滚。
