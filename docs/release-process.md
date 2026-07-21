# 发布流程与版本号策略

本文规定唯一的产品版本号来源、升版步骤和发布产物验证方式。日常运维见
[operations.md](operations.md)，安装与激活见
[../deploy/README.md](../deploy/README.md)。

## 1. 单一版本号

产品版本号只有一个权威来源：`backend/rust/Cargo.toml` 的
`[workspace.package] version`。它从这里传播到所有对外可见的位置：

- 全部 workspace crate 通过 `version.workspace = true` 继承;crate 之间的路径
  依赖只写 `{ path = "../..." }`,不重复声明版本号,升版不需要在这些依赖行上
  做任何改动。
- 前端 7 个 `package.json`（`frontend/`、`frontend/apps/{user,admin}`、
  `frontend/packages/{api-client,config,i18n,types}`）的 `version` 字段人工保持
  同值。前端包互相通过 `workspace:*` 引用，版本字段纯粹是标识，不参与解析。
- API 的 `/metrics` 输出 `v2board_build_info{crate_version="X.Y.Z"} 1`
  （编译期 `CARGO_PKG_VERSION`，随 workspace 版本自动变化）。
- 原生发布包的 `RELEASE` 元数据文件带 `version=X.Y.Z` 行，由
  `Dockerfile.rust` 的装配阶段从 workspace manifest 读出并盖章；
  `v2board-lifecycle inspect-release-archive` 校验该行格式并在 JSON 结果中以
  `version` 字段回显。

版本语义遵循 SemVer。当前处于 `0.x` 预发布阶段：破坏性变更或功能波次升 minor，
纯修复升 patch。首次正式生产安装时升到 `1.0.0`。

## 2. 升版步骤

一次升版是一个提交，包含以下全部内容：

1. 改 `backend/rust/Cargo.toml` 的 `[workspace.package] version`。
2. 在 Docker 内重新生成 `Cargo.lock`（rust 源码在容器里是只读挂载，需要先拷到
   容器内可写目录）：

   ```sh
   docker compose -f docker-compose.local.yml run --no-deps --rm -T rust-api sh -c \
     'cp -r /src/backend/rust /tmp/rust-lock && cd /tmp/rust-lock \
      && cargo update --workspace --offline 1>&2 && cat Cargo.lock' \
     > /tmp/Cargo.lock.new
   cp /tmp/Cargo.lock.new backend/rust/Cargo.lock
   ```

   随后 `git diff backend/rust/Cargo.lock` 应当只包含 workspace crate 的版本行。
3. 同步改 7 个前端 `package.json` 的 `version` 字段。
4. 更新根目录 `CHANGELOG.md`：把 `[Unreleased]` 下积累的条目移入
   `[X.Y.Z] - 日期` 小节，保留空的 `[Unreleased]`。
5. 验证：`make rust-check`、`make rust-test`、`make native-release-audit`，前端
   改动后 `make sync` 加相关 typecheck。
6. 直接提交到 main。打 git 标签不是自动的：只在真正裁切一次发布时人工执行
   `git tag vX.Y.Z && git push origin vX.Y.Z`，日常版本号提交不打标签。

## 3. 发布产物与验证

CI（或本地 `docker build --target native-release`，带 40 位小写十六进制
`V2BOARD_SOURCE_REVISION` 构建参数）产出
`v2board-native-debian-13-amd64.tar.gz`。包内 `RELEASE` 恰好 7 行：

```text
format=v2board-native-release-v1
version=X.Y.Z
source_revision=<40 位 git sha>
target_os=linux
target_distribution=debian
target_distribution_version=13
target_arch=amd64
```

装配阶段强制 `wc -l = 7` 且版本非空；`SHA256SUMS` 覆盖 `RELEASE` 本身，任何
篡改都会被 `sha256sum --check` 与 `inspect-release-archive` 拒绝。安装前按
[../deploy/README.md](../deploy/README.md) 验证外部摘要、内部校验和，并确认
`RELEASE` 的 `version` 与 `source_revision` 是预期值。上线后用
`curl -s http://127.0.0.1:8080/metrics | grep v2board_build_info`（本机回环）
确认运行中的二进制就是该版本。

## 4. CHANGELOG 维护

- 面向对外可见行为：运维、操作者或用户能感知的变更才进 CHANGELOG；纯内部
  重构、测试调整留在 git 记录里。
- 条目挂在 `[Unreleased]` 下，按 Keep a Changelog 的
  `Added / Changed / Fixed / Removed / Security` 分类。
- 升版时随第 2 节步骤 5 一起裁切。
