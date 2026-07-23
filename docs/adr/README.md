# 架构决策记录(ADR)

本目录收录 V2Board Native 的架构决策记录。**0001–0007 是回填的既成决策**:
它们记录的选择早已在代码、文档和门禁中落地,写成 ADR 是为了让"为什么是
这样"可追溯,而不是重新开放讨论。每篇的 Context 描述当时的约束,Decision
写清选了什么、明确拒绝了什么,Consequences 诚实记录代价。

**新的架构决策应当先写 ADR 再实现**:复制 [`template.md`](template.md),按
下一个可用编号命名为 `NNNN-短横线-slug.md`,在同一提交系列里携带实现(或在
实现前单独提交并标记状态为"提议")。修订既有决策时不改写旧文,而是新增一篇
并在旧文顶部标注被取代。

各篇内容以仓库内既有文档与代码为事实依据;当 ADR 与
[`../api-dialect.md`](../api-dialect.md)、各 invariants 文档或根目录
`AGENTS.md` 冲突时,以后者为准并修正 ADR。

## 索引

| 编号 | 标题 | 状态 |
| --- | --- | --- |
| [0001](0001-single-host-debian-systemd-cloudflare-tunnel.md) | 单机 Debian 13 + systemd 直跑原生二进制,Cloudflare Tunnel 为唯一公网入口 | 已采纳(回填) |
| [0002](0002-postgres-clickhouse-redis-split.md) | PostgreSQL(业务)+ ClickHouse(可牺牲分析)+ Redis(运行态)三库分工 | 已采纳(回填) |
| [0003](0003-opaque-session-tokens-redis-session-epoch.md) | 不透明会话 token + Redis 存储 + `session_epoch` 吊销锚点 | 已采纳(回填) |
| [0004](0004-api-dialect-v2-problem-json.md) | 内部 API 方言现代化:RFC 9457 problem+json 与外部命名空间字节冻结 | 已采纳(回填) |
| [0005](0005-one-shot-mysql-import-copy-stream.md) | 一次性 MySQL 导入:只读快照 + COPY 流式,失败即弃重来 | 已采纳(回填) |
| [0006](0006-frontend-contract-tiers-interaction-parity.md) | 前端行为契约分层(Tier-1/Tier-2),像素对比退役,交互 parity 常设 | 已采纳(回填) |
| [0007](0007-payment-config-aes-gcm-envelope.md) | 支付网关配置静态加密:AES-256-GCM 信封与确定性 nonce | 已采纳(回填) |
| [0008](0008-clickhouse-write-without-read-launch-debt.md) | 首发接受 ClickHouse 只写不读:analytics reader 无生产调用方 | 已采纳(记录已知技术债) |
| [0009](0009-hand-written-openapi-codegen-vs-off-the-shelf.md) | 维持自研 OpenAPI→TypeScript+Zod 生成器,否决现成工具替换 | 已取代(由 0010) |
| [0010](0010-hey-api-openapi-ts-component-schema-codegen.md) | 组件 schema 生成改用 @hey-api/openapi-ts,运行时算子注册表继续自研 | 已采纳 |
