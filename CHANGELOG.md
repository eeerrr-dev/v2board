# 更新日志

本项目所有对外可见的显著变更记录于此文件。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循
[语义化版本](https://semver.org/lang/zh-CN/)。版本号的唯一权威来源是
`backend/rust/Cargo.toml` 的 `[workspace.package] version`；升版流程见
[docs/release-process.md](docs/release-process.md)。

## [Unreleased]

### Added

- 文档级站点标识：服务端按运营方配置渲染 HTML 标题、meta 描述与用户端的
  canonical / Open Graph 标签；新增固定公共路由 `/robots.txt`；两个应用带
  SVG favicon；管理端 HTML 恒带 `noindex`。

## [0.9.0] - 2026-07-20

首个统一版本基线。产品尚未正式发布安装（预发布，`0.x`）；此前的完整开发历史见
git 记录，不在此逐条回溯。

### Added

- Rust 原生后端（API、worker、analytics）与 PostgreSQL / ClickHouse / Redis
  运行时，systemd 部署，Cloudflare Tunnel 作为唯一公网入口。
- 重新设计的 shadcn 用户端与管理端前端，内部 API 方言全面现代化
  （`docs/api-dialect.md` W0–W14）。
- 一次性 MySQL → PostgreSQL 导入工具 `v2board-lifecycle`。
- 统一产品版本号：全部 workspace crate 与前端包继承同一版本；
  `/metrics` 的 `v2board_build_info{crate_version}`、原生发布包 `RELEASE` 的
  `version=` 行、`v2board-lifecycle inspect-release-archive` 输出的 `version`
  字段均报告该版本。
