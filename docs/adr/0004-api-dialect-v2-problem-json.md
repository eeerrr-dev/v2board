# 0004. 内部 API 方言现代化:RFC 9457 problem+json 与外部命名空间字节冻结

- 状态:已采纳(回填)
- 日期:2026-07(回填;Appendix A 波次 W0–W14 已全部落地)

## 背景(Context)

从 Laravel 继承的内部方言充满事故温床:`{data}` 信封、错误靠中文 message
字符串精确匹配来分流(改一个文案就破坏前端逻辑)、bracket 风格
`filter[i][...]` 查询参数、hash 路由、以及混进 500 的确定性业务拒绝。同时,
一批**真实外部方**依赖既有字节:订阅客户端、节点代理、支付网关回调、
Telegram webhook、Stripe/reCAPTCHA 集成 —— 它们不升级、不协商,
改一个字节就是生产事故。

## 决策(Decision)

内部与外部一刀两断,以 `docs/api-dialect.md` 为唯一权威:

- **内部路由**(passport/auth、user、动态前缀下的 admin、guest comm/config)
  全部现代化:RFC 9457 `application/problem+json` 错误体 + 稳定 snake_case
  `code` registry(前端唯一判别键)、JSON body + `Authorization: Bearer` +
  `Accept-Language`、无 `{data}` 信封、`{items,total}` 分页、RFC 3339 时间戳、
  JSON filter DSL、checkout 判别联合、history 路由 + 客户端
  `legacy_hash_redirect_enable` 翻译器、`custom_html` 移除 + CSP 收紧。
- **外部命名空间字节冻结**(§2 清单):`/api/v1/client/*`、
  `/api/v1/server/{class}/{action}`、`/api/v2/server/config`、
  `/api/v1/guest/payment/notify/{method}/{uuid}`、
  `/api/v1/guest/telegram/webhook`、订阅 URL/token/flag 方案、集成 payload、
  `localStorage.authorization` 键。旧 `{message}` 错误体和响应重写本地化中间件
  只为这些命名空间保留。
- **逐族原子切换**:每个端点族的后端 + 前端 + api-client + fixtures + 场景 +
  goldens 在同一提交系列翻转(Appendix A 记录每一波);wire 由 golden lane
  双端钉住(`crates/api/src/golden_wire.rs` 与
  `packages/api-client/src/goldens.test.ts`)。

**明确拒绝的替代方案:**

- **双方言兼容层 / 版本协商 / 长期并行** —— 两套构造器意味着每个错误路径都要
  测两遍且永远删不掉;W14 已把 legacy 内部信封构造器物理删除,兼容分支不得
  回归。
- **给外部命名空间"顺便"现代化** —— 外部方不可协调,冻结是唯一安全选择。
- **继续用错误文案做机器判别** —— 已被 `code` 取代并禁止回归。

## 后果(Consequences)

- 得到:前端错误分流只看 `code`,文案可自由本地化;wire 形状有 golden 测试
  双端锁定;404/401/403/422 语义与 HTTP 对齐(401 + `session_expired` 才拆
  会话);规范即契约,内部形状的疑问查 spec 而不是考古旧代码。
- 代价:**破坏第三方 in-app 登录**(passport 现代化的已接受 owner 决策);
  任何内部契约改动都必须走 spec 修订 + golden 更新,不能"顺手改"。
- 代价:代码里长期共存两套响应模型(`compat` crate 的 problem+json 与 legacy
  封装),响应重写本地化中间件作为外部专用件永久保留 —— 这是冻结的持续
  维护成本。

## 证据

- [`../api-dialect.md`](../api-dialect.md) — 方言唯一权威(§2 冻结清单、§3 错误
  模型、Appendix A/B 波次与修订记录)。
- [`../../backend/rust/crates/api/src/golden_wire.rs`](../../backend/rust/crates/api/src/golden_wire.rs)
  与
  [`../../frontend/packages/api-client/src/goldens.test.ts`](../../frontend/packages/api-client/src/goldens.test.ts)
  — 双端 golden wire lane。
- [`../../backend/rust/crates/compat/src/problem.rs`](../../backend/rust/crates/compat/src/problem.rs)
  — problem+json 实现;同 crate 保留外部 legacy 响应封装。
- [`../../AGENTS.md`](../../AGENTS.md) — Internal API Dialect Direction。
