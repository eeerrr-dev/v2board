# Dev Container（仅编辑器）

此 Dev Container 为 VS Code 提供与项目一致的 Rust、rust-analyzer、Node 24、
TypeScript、ESLint 与 Tailwind 语言服务。它不替代根目录的 `make` 工作流；运行、构建、
测试和部署仍由 `docker-compose.local.yml` 完成。

## 使用

1. 安装 VS Code **Dev Containers** 扩展。
2. 运行 **Dev Containers: Reopen in Container**。
3. 首次创建时，`post-create.sh` 会执行锁定的 pnpm install 和 cargo fetch。

仓库以读写方式挂载到 `/workspaces/v2board`，源码编辑正常写回宿主机。以下输出被独立
named volumes 遮盖，不会污染宿主工作树：

- frontend 根、两个 app 与四个 package 的 `node_modules`；
- pnpm content store；
- Cargo registry、git checkout 与 target。

Dev Container 使用独立 Compose project，不会碰触 `make up` 的 PostgreSQL、ClickHouse、Redis、
Rust runtime 或 frontend deploy volumes。需要验证修改时仍应运行 `make sync`，然后使用
`make doctor`、`make rust-check`、`make deploy-smoke` 等项目 gate。
