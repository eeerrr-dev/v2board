# 0007. 支付网关配置静态加密:AES-256-GCM 信封与确定性 nonce

- 状态:已采纳(回填)
- 日期:2026-07(回填;决策早于此日期落地)

## 背景(Context)

`payment_method.config`(JSONB)保存各支付网关的密钥材料(API key、webhook
secret 等),此前以明文对象落库 —— 数据库备份、PITR 副本和任何拿到 SQL 读
权限的路径都会直接暴露 provider 凭据。约束:

- 单机部署,没有(也不想引入)外部密钥管理服务;运行时已有一个必须妥善
  保管的根秘密 `app_key`,operator config 的四个 secret 也用它派生的 key
  加密;
- MySQL 一次性导入要求**字节确定性**:同一清单重跑必须产出相同的目标行,
  转换期累计的 canonical expectation 要与 COPY 后整表扫描逐字节对上
  (见 ADR 0005);随机 nonce 会让每次加密结果不同,直接破坏这条链;
- 首发前没有存量明文行,不存在"平滑迁移"问题。

## 决策(Decision)

`payment_method.config` 列改存 version-1 **AES-256-GCM 信封**
(`{format_version, nonce, ciphertext, tag}`,由 `payment-adapters` 的专用
at-rest adapter 实现):

- **密钥自 `app_key` 派生**,使用支付专属 domain 常量
  (`v2board/payment-config/key/v1`),与 operator-config 加密 key 域分离;
- **AAD 绑定网关 driver + 行 `uuid`**,信封不能在行与行、driver 与 driver 之间
  搬移重放;
- **确定性 nonce**:对 (domain, driver, uuid, plaintext) 做 keyed PRF 取 12 字节。
  给定 (key, nonce) 只会加密这一份明文,满足 GCM 无随机 nonce 时的唯一安全
  前提;明文经 sorted-key JSON 规范化,同一配置永远得到同一信封 —— 导入转换、
  canonical row hash 与 COPY 后校验扫描因此可复现,信封相等 ⇔ 配置相等;
- **无明文兜底**:读到非信封形状的存量值是硬完整性错误,不是兼容回退。

**明确拒绝的替代方案:**

- **明文兜底 / 双格式过渡期** —— 首发前没有存量行,兜底只会留下一条永远
  删不掉的降级路径;
- **外部 KMS / HSM** —— 单机单操作者引入网络型密钥服务是新的可用性单点与
  运维面,而威胁模型(数据库转储泄露)用本地信封已覆盖;
- **随机 nonce** —— 密码学上更常规,但破坏导入的字节确定性与信封等价比较;
  确定性 nonce 的安全前提(一 key 一 nonce 一明文)已由构造保证并写入代码
  注释。

## 后果(Consequences)

- 得到:数据库转储/备份不再含明文支付凭据;信封被换行、换 driver 时解密即
  失败;导入可复现性与不可变 checkout 快照比较得以保持。
- 代价:**`app_key` 升格为不可丢失的长期密钥** —— 丢了它,加密的 provider
  secret 无法恢复(运维备份表已按此告警);轮换 `app_key` 意味着重加密所有
  信封,目前没有自动轮换工具。
- 代价:确定性加密天然泄露"相等性" —— 能看出两行配置相同,以及同一行的
  配置何时发生过变化;对这类低熵有限集合的字段这是已接受的权衡。
- 代价:管理端读配置必须走脱敏(`********` 哨兵回写保留原 secret),任何新
  读取路径都要记得解密与脱敏两步。

## 证据

- [`../../backend/rust/crates/payment-adapters/src/payment_secrets.rs`](../../backend/rust/crates/payment-adapters/src/payment_secrets.rs)
  — 信封实现:key/nonce/AAD 派生域、确定性理由注释、无明文兜底。
- [`../../backend/rust/crates/lifecycle/src/mysql_import/copy_stream.rs`](../../backend/rust/crates/lifecycle/src/mysql_import/copy_stream.rs)
  — 导入器使用 canonical 加密入口保持逐字节确定。
- [`../operations.md`](../operations.md) §2 — "`app_key` 丢失使加密的 provider
  secret 不可恢复"的备份要求。
- [`../../README.md`](../../README.md) — operator secret 的同族 AES-256-GCM
  处理(`server_token`、SMTP、Telegram、reCAPTCHA)。
- [`../../backend/README.md`](../../backend/README.md) — Admin config 读脱敏与
  `********` 哨兵语义。
