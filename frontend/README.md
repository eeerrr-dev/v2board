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
  previous -> releases/<previous-content-id>
```

每个 manifest 只有一个入口，所有 JS/CSS/字体/图片以扁平的内容哈希文件名发布。HTML 中只
保留严格校验的 runtime-config token；Rust 在请求时注入品牌、语言和动态后台路径。
发布使用原子 symlink 切换，并只保留 current/previous 两代，因此失败构建不会破坏最后
一个可用 release，滚动发布间的在途静态请求也可以从 previous 完成。

运行 `make deploy-smoke` 验证 release 结构、Rust HTML 注入、资源路由和旧 bundle
隔离；运行 `make visual-smoke` 用 Docker 内 Chromium 检查桌面与移动布局。

## 行为与参考项目

`make interaction-parity`（`make behavior-parity` 的实现）驱动当前 Rust + React 应用
和 `references/wyx2685-v2board` 中的只读参考 UI。比较范围只保留真实 Tier-1 契约：
API endpoint/payload、认证与语言持久化、外部链接/集成及安全跳转。参考资源从不进入
Vite 输入、deploy 输出或生产镜像。

```bash
make reference-oracle-check
make interaction-parity
make accessibility-smoke
make parity-config-audit
make ui-sync-audit
```

像素截图 lane 已退役。测试关注行为、可访问性、shadcn/Radix 结构和真实契约，不保留
Ant Design、Bootstrap、OneUI、旧全局 CSS 或固定 `umi.js`/`umi.css` 入口。

## 约束

- strict TypeScript；不使用 `any` 逃逸。
- HTTP 请求统一经过 `@v2board/api-client`。
- TanStack Query 管理 server state，React Router 管理路由状态。
- user/admin 使用本地拥有的 shadcn primitives、Radix 与 `lucide-react`。
- 两个应用的共享 UI primitive 和全局 token CSS 必须通过 `make ui-sync-audit`。
- 不复制、不拼接、不加载参考项目中的旧 bundle、CSS、字体或主题资源。
