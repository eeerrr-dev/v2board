# 0006. 前端行为契约分层、source-first 交互门禁与有限 legacy oracle

- 状态:已采纳(回填)
- 日期:2026-07(回填;全部 visual 场景已标记 `visualRetired`)

## 背景(Context)

前端从旧打包产物(Ant Design v3 / Bootstrap / OneUI / umi bundle)整体重建为
shadcn/Radix 设计。重建期间需要回答:"和旧站不一样"什么时候是 bug,什么时候
是设计?早期的像素截图对比把每个渲染字节都当契约,导致:

- 任何有意的重设计都要伴随大量截图基线更新,噪声淹没真信号;
- 真正不可破坏的东西(外部方消费的 URL、持久化键、安全跳转结果)和纯表现
  选择(toast 时机、日期格式)混在同一个"必须一致"的桶里;
- 参考实现只是只读 witness,把它的 DOM 当标准会反向锁死重设计。

## 决策(Decision)

行为契约分两层,锚点是 Rust 后端与外部集成,**不是参考前端**:

- **Tier-1(不可动,永久)**:真实外部方消费的契约 —— 字节冻结的外部命名空间
  与集成 payload、历史路由路径 + `legacy_hash_redirect_enable` 翻译器、后端
  铸造的路径式 URL(`?verify=` 邮件登录、`{app_url}/order/{trade_no}` 支付
  返回)、`localStorage.authorization` 持久化键、导入数据解释(公告 `弹窗`
  标签、知识库 `copy()`/`jump()` 钩子),以及安全/会话关键**结果**(401 +
  `session_expired` 才拆会话、跨账号缓存隔离、no-credentials CORS、服务端
  注册强制、i18n 持久化)。
- **Tier-2(保守钉住,可按面放宽)**:无外部方消费的表现层 —— 展示格式、
  spinner/toast/轮询/refetch 时机、弹窗 vs 移动导航等。重设计的面允许 owner
  有意识地改 Tier-2 并更新/退役对应场景,前提是 Tier-1 完好且路由仍有行为
  场景覆盖;拿不准就按 Tier-1 处理。
- **像素/视觉 parity 全面退役**(每个场景 `visualRetired: true`);
  `make interaction-parity`(Playwright Test)是 source-world 常设门禁,
  新功能只对当前产品行为负责。旧只读参考被隔离到显式的
  `make legacy-oracle-parity` 迁移审计,不再是每次产品变更的默认依赖。
  `make real-stack-e2e` 另以真实浏览器、Rust API 和受限 PostgreSQL/Redis
  runtime principal 关闭模拟 fixture 无法覆盖的跨层缝隙。内部 wire 形状
  仍由 `docs/api-dialect.md` + golden lane 钉住。

**明确拒绝的替代方案:**

- **字节级 DOM/像素复刻** —— 把重设计判死刑,且维护成本随分辨率×主题×语言
  组合爆炸;20k 行像素 harness 已删除。
- **"全都算契约"的扁平清单** —— 没有分层就没有可放宽的合法途径,实际结果是
  测试被静默删除而不是有记录地退役。
- **以参考前端为锚** —— 参考只是 witness,匹配它是匹配真实契约的代理手段,
  永远不是目的。

## 后果(Consequences)

- 得到:重设计有了明确的合法空间和退役程序;默认门禁不再要求旧 UI 与新产品
  同步演进;参考实现保持只读且不进任何构建产物。
- 代价:分层判断需要人:每个边缘案例都要问"谁消费它",判断错向(把 Tier-1
  当 Tier-2 放宽)就是真实故障,所以规则要求存疑从严。
- 代价:legacy lane 仍需维护既有 union 选择器和 Tier-1 reducer,但只在修改冻结
  兼容契约时运行;新增普通场景不得扩大该维护面。场景清单变更仍需经过
  `make parity-config-audit`。
- 代价:放弃了"任何视觉回归自动可见"的安全网,视觉质量改由
  `make visual-smoke` 人工浏览器烟测与无障碍门禁兜底。

## 证据

- [`../../AGENTS.md`](../../AGENTS.md) — Frontend Contract Direction(Tier 模型
  的规范表述)与各 surface direction 清单。
- [`../../frontend/README.md`](../../frontend/README.md) — 行为与参考项目章节
  (比较范围只保留真实 Tier-1 契约)。
- [`../../frontend/tests/`](../../frontend/tests/) — source-first Playwright
  harness、显式 legacy 适配层与不安装 API fixtures 的 `real-stack/` 旅程。
- [`../api-dialect.md`](../api-dialect.md) §2、§13 — 冻结外部契约与
  canonical-semantics 适配器。
