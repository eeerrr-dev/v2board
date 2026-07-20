# V2Board Frontend

React 19、TypeScript、Vite、Tailwind v4 与 shadcn/Radix 组成的 pnpm monorepo。
用户端和管理端都由源码构建，不读取旧打包 CSS/JS，也不依赖 PHP 模板。

## 目录

```text
apps/
  user/         用户端纯 shadcn 应用
  admin/        管理端纯 shadcn 应用
packages/
  api-client/   类型化 API 边界
  config/       Vite、Tailwind 与 TypeScript 共享配置
  i18n/         i18next 初始化与翻译资源
  types/        共享领域类型
scripts/        deploy、浏览器 smoke 与审计脚本
tests/          Playwright 行为/无障碍契约
```

## 开发

从仓库根目录使用 Docker 工作流：

```bash
make up
make sync
make doctor
```

- <http://localhost:5173>：用户端 Vite server。
- <http://localhost:5174/admin>：管理端 Vite server。
- <http://localhost:8000>：Rust 提供的 production-shaped 页面与 API。

两个 Vite server 都将 API 请求代理到 `rust-api:8080`。不要在宿主机运行会产生
`node_modules`、`.pnpm-store`、`dist`、缓存或报告的 pnpm/npm 命令。

需要直接执行 workspace 命令时，在容器内运行：

```bash
docker compose -p v2board -f docker-compose.local.yml exec frontend pnpm typecheck
docker compose -p v2board -f docker-compose.local.yml exec frontend pnpm test
```

命令行 typecheck/build 使用原生 TypeScript 7 的 `tsc`；ESLint 与其他仍依赖稳定
compiler API 的工具按 TypeScript 官方迁移方案并行使用 `@typescript/typescript6`。等
原生 compiler API 稳定后再移除兼容包，而不是让生产检查继续停在旧编译器上。

## 部署格式

`pnpm build:deploy` 是唯一受支持的部署构建。它先在 release staging 目录中同时
typecheck、构建并验证两个应用，成功后才发布：

```text
dist-deploy/
  releases/<content-id>/
    user/{index.html,manifest.json,<content-hashed files>}
    admin/{index.html,manifest.json,<content-hashed files>}
  current -> releases/<content-id>
  previous -> releases/<previous-content-id-or-current-on-first-publication>
```

每个 manifest 只有一个入口，所有 JS/CSS/字体/图片以扁平的内容哈希文件名发布。HTML 中只
保留严格校验的 runtime-config token；Rust 在请求时注入品牌、语言和动态后台路径。`content-id`
按两个应用全部已验证文件的规范路径和真实字节计算，不只信任 Vite 文件名或 manifest/index 文本。
发布使用原子 symlink 切换，并只保留 current/previous 两代，因此失败构建不会破坏最后
一个可用 release，滚动发布间的在途静态请求也可以从 previous 完成。首次发布没有真实旧代，两个
链接都指向同一不可变 content ID；第二次发布后 `previous` 才代表真实上一代。这里的 fallback 只覆盖
单个 frontend tree，不构成 outer native release、跨实例或集群回滚保证。

运行 `make deploy-artifact-smoke` 只构建并验证 release 结构和旧 bundle 隔离，不启动
默认 PostgreSQL/ClickHouse/Redis/Rust 服务；`make deploy-smoke` 在这个产物门禁之后再
验证 Rust HTML 注入和资源路由。运行 `make visual-smoke` 用 Docker 内 Chromium 检查
桌面与移动布局。

## 行为与参考项目

`make interaction-parity` 驱动当前 Rust + React source world，是日常行为门禁。
只读参考 UI 已隔离为按需的 `make legacy-oracle-parity`，比较范围只保留真实
Tier-1 契约。`make real-stack-e2e` 不安装 API fixture，也不启动默认 runtime/data
服务或挂载普通业务数据卷；它复用 frontend/Rust 构建缓存、依赖、测试报告和已
验证的 deploy artifact，并使用真实 Rust 和 tmpfs 隔离的受限 PostgreSQL/Redis
runtime role 验证登录/页面、配置写入，以及套餐价格 Retain/Clear/Set 的关键
旅程。用于容器间传递 API 测试凭据的专用 `real-stack-e2e-api-runtime` 卷会在旅程前后
清空；只读浏览器 runner 不挂载 deploy 卷，worker 配置只校验、不落盘。参考资源
从不进入 Vite 输入、deploy 输出或生产 release artifact。

```bash
make reference-oracle-check
make deploy-artifact-smoke
make interaction-parity
make legacy-oracle-parity     # 仅兼容契约变更
make real-stack-e2e
make accessibility-smoke
make parity-config-audit
make ui-sync-audit
```

像素截图 lane 已退役。测试关注行为、可访问性、shadcn/Radix 结构和真实契约，不保留
Ant Design、Bootstrap、OneUI、旧全局 CSS 或固定 `umi.js`/`umi.css` 入口。

## 约束

- strict TypeScript；不使用 `any` 逃逸。
- HTTP 请求统一经过 `@v2board/api-client`。
- 全部 158 个唯一内部 API operation 由 Rust 注册表投影为 OpenAPI、TypeScript 与
  Zod，使用 `make api-contract-check` 校验 operation 集合、RFC 9457 body 中 101 个
  应用级 problem code/status/title tuple 及生成漂移。注册表覆盖 method/path、
  path/query 参数、公共及 operation-specific headers、请求体存在性、鉴权和精确成功
  状态/媒体；允许同一 operation 明确声明多个 2xx/3xx 状态或媒体类型，但禁止状态
  通配。采用单一成功状态的生成 endpoint 会在 Zod 解析响应体前校验该状态。该门禁
  保证 operation inventory 与上述 operation-level metadata 全量；多数尚未迁入 Rust
  `api-contract` 的 JSON 请求及成功响应仍是 `JsonValue`，不能描述成字段级全量生成。
  wire DTO、页面模型和 form draft 不共用同一类型。
- TanStack Query 管理 server state，React Router 管理路由状态。
- user/admin 通过 `@v2board/ui` 共享 shadcn primitives、token CSS 与平台工具；
  各自只保留产品专用 composition，并统一使用 Radix 与 `lucide-react`。
- `make ui-sync-audit` 保证共享 primitive 及其组件/hook/platform 测试只有一个
  canonical 所有者，且应用目录不重新出现副本；产品专用组件测试仍留在各应用。
- 不复制、不拼接、不加载参考项目中的旧 bundle、CSS、字体或主题资源。
