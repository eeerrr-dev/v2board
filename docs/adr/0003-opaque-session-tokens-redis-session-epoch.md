# 0003. 不透明会话 token + Redis 存储 + `session_epoch` 吊销锚点

- 状态:已采纳(回填)
- 日期:2026-07(回填;决策早于此日期落地)

## 背景(Context)

会话设计的硬约束:

- 改密码、封禁、管理员重置**必须立刻让该用户所有已发放会话失效** —— 这是
  安全结果,不是可选优化;
- 浏览器持久化键 `localStorage.authorization` 及其存储值格式是冻结的外部
  契约(存原始 token,`Bearer` 只在 wire 上加);
- 管理/staff 的特权写操作需要"近期密码复核"(step-up)这层额外门;
- 生产是单机,不存在跨服务无状态验签的需求;Redis 本来就在栈里。

## 决策(Decision)

- 登录签发 **256-bit 不透明 token**:`getrandom` 取 32 字节,base64url 无填充
  编码（`auth-adapters` 实现 application 的随机凭据 port）。
- **Redis 只存 SHA-256(token) 十六进制查找键**(带绝对 TTL),不存明文 token;
  会话记录含用户、`session_epoch`、签发 IP/UA、`password_authenticated` 等。
- **`users.session_epoch`(PostgreSQL)是持久吊销锚点**:改密码、封禁、staff/
  管理员重置和显式吊销会删除查找键并推进 epoch;即便 Redis 键漏删,epoch 不匹
  配的会话也直接作废 —— 全量下线由数据库事实兜底。
- **特权会话更短 TTL + step-up**:`POST /api/v1/auth/step-up` 密码复核后授予
  短期特权标记;401 + `session_expired` 才触发前端会话拆除,403
  `permission_denied`/`step_up_required` 绝不拆会话(方言 §3.2 固定)。
- MySQL 导入**不迁移任何旧会话**,上线后全员重新登录;native 运行时不含旧
  JWT 解码器或 query/form 认证回退。

**明确拒绝的替代方案:**

- **JWT / 无状态签名会话** —— 无法即时吊销是硬伤;"短寿命 + 刷新 token"只是把
  吊销延迟摊薄,不满足"改密码立刻全下线"。单机架构下无状态验签没有收益。
- **会话放 PostgreSQL 热路径** —— 每请求一次事务库查找没有必要,Redis 丢失的
  代价(全员重登)已被明确接受。
- **保留旧站 token/JWT 兼容解码** —— 破坏第三方 in-app 登录是已接受的 owner
  决策,不留双认证路径。

## 后果(Consequences)

- 得到:吊销即时且完整(Redis 删键 + epoch 双保险);Redis 泄露只暴露哈希,
  拿不到可用 token;step-up 把"已登录"与"最近证明过密码"分成两个明确状态。
- 代价:Redis 成为登录可用性依赖 —— 它宕机时认证按 fail-closed 契约不可用;
  每个认证请求多一次 Redis 往返。
- 代价:换机器/清 Redis 等于全员重新登录(已接受,导入契约本身就如此);
  没有跨实例无状态验证能力,若未来多机部署需重新评估。
- 代价:第三方基于旧 token 语义的 in-app 登录已破坏(接受的破坏,记录在
  方言文档 owner 决策里)。

## 证据

- [`../../backend/rust/crates/application/src/auth.rs`](../../backend/rust/crates/application/src/auth.rs)
  — 会话签发、epoch 校验、step-up 与吊销策略。
- [`../../backend/rust/crates/auth-adapters/src/external.rs`](../../backend/rust/crates/auth-adapters/src/external.rs)
  与 [`../../backend/rust/crates/auth-adapters/src/cache.rs`](../../backend/rust/crates/auth-adapters/src/cache.rs)
  — 256-bit token 生成、SHA-256 Redis 查找键和原子会话脚本。
- [`../../backend/rust/crates/db/src/auth.rs`](../../backend/rust/crates/db/src/auth.rs)
  — PostgreSQL 账户及 `session_epoch` 持久化 port 实现。
- [`../../backend/README.md`](../../backend/README.md) — Authentication and
  operational contracts 章节。
- [`../api-dialect.md`](../api-dialect.md) §3.2(401/403 语义)、§5.2(auth 路由
  与 quick-login/token-login 拆分)。
- [`../../backend/rust/crates/api/src/routes.rs`](../../backend/rust/crates/api/src/routes.rs)
  — `/api/v1/auth/step-up` 路由注册。
