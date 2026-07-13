# MySQL 一次性导入指南

本文说明如何把固定旧版的 Oracle MySQL 8 数据一次性导入尚未发布的 native 新版。数据规则见
[MySQL 一次性导入不可变契约](mysql-import-invariants.md)，数据库长期运行规则见
[PostgreSQL + ClickHouse 持久化不变量](postgresql-clickhouse-invariants.md)。

导入只有一条路径：

```text
停止旧写入
  → 导出完整 MySQL dump
  → 导入一次性 MySQL 8 engine
  → 转换到全新 PostgreSQL
  → 以空 ClickHouse 和空 Redis 启动
  → 生成新配置
  → 验证并启动新版
```

旧 MySQL 不被修改。staging 只是处理 dump 的临时输入，不是恢复旧库，也不会成为新版运行时。
当前设计确实需要在迁移期间临时运行一个 MySQL 8 engine，因为 converter 读取的是加载到 staging
后可查询的旧 schema，而不是直接解析 dump SQL；这不等于在新生产服务器永久安装 MySQL。

## 1. 准备

准备以下资源：

- 与固定旧版 schema 匹配的 Oracle MySQL 8.0/8.4；
- 一个完整 MySQL dump；
- 一个新的、一次性的 staging MySQL 8 engine/database；
- 一个全新的 PostgreSQL 18 database 和分离的 migration/API/worker principals；
- 一个没有旧事件的 ClickHouse 26.3 database；
- 一个全新的空 Redis；
- 新 API、worker 和 frontend release；
- [`mysql-import.v1.example.json`](examples/mysql-import.v1.example.json) 的私有副本。

不要提供旧 Redis URL、Stripe credential、旧 systemd unit 或旧 MySQL live URL。导入器不需要它们。

## 2. 停止旧站写入

在生成 dump 前停止所有会修改旧 MySQL 的来源：

- PHP/API 和管理后台；
- scheduler、queue worker 和定时任务；
- 新订单与支付入口；
- 仍向旧站上报流量的远端节点。

这一步由操作者在导入器外完成。导入器不会控制旧服务，也不会修改旧数据库来制造写入 fence。

## 3. 导出完整 dump

使用 MySQL 8 `mysqldump` 导出全部业务表 schema 和全部行，并计算文件 SHA-256。不要导出或在
staging 执行旧 trigger、routine 或 event。示意命令：

```bash
umask 077
mysqldump \
  --single-transaction \
  --quick \
  --hex-blob \
  --default-character-set=utf8mb4 \
  --no-tablespaces \
  --set-gtid-purged=OFF \
  --skip-triggers \
  v2board > /secure/private/v2board-legacy.sql

sha256sum /secure/private/v2board-legacy.sql
```

使用只读导出凭据，不执行任何 `ALTER`、`UPDATE`、`DELETE` 或 schema 修改。dump 应是受限权限的
regular file；如何做离线备份或静态加密属于操作者的文件安全措施，不扩展导入清单格式。

## 4. 创建一次性 staging MySQL

创建一个全新的空 MySQL 8 database，把 dump 导入其中。这个临时 engine 可以部署为：

- 迁移机或新机上的 one-off container；
- 一台用完即删的临时 VM；
- 受控私网内、converter 可访问的其他一次性主机。

它不需要运行在旧服务器，也不需要作为新服务器的 system service 永久安装。这个 database：

- 只供 converter 读取；
- 不接收用户流量；
- 不与旧 MySQL 同步；
- 不保存导入进度；
- 完成或失败后都可以直接丢弃。

导入结束后停止并删除 engine、container/VM 和临时 volume。新版长期运行只需要 PostgreSQL、
ClickHouse 和 Redis。

如果 staging 不是本机受控维护网络，连接必须验证 TLS 主机身份。导入后先验证 server vendor/version
和固定旧 schema；任何未知表结构、类型、索引或关系差异都停止转换。

## 5. 填写 `mysql-import.v1`

复制示例：

```bash
cp docs/examples/mysql-import.v1.example.json /secure/private/mysql-import.json
chmod 600 /secure/private/mysql-import.json
```

只填写三部分：

- `source`：dump path、dump SHA-256、staging MySQL URL 和 transport security；
- `target`：全新 PostgreSQL、ClickHouse、Redis 连接及两个配置文件路径；
- `runtime`：新版完整 typed 配置。

`schema_version` 固定为 `1`。保留/丢弃规则和 Stripe/Redis 处置不允许逐次选择；这些行为已经由
converter 和不可变契约固定。

URL 中的用户名、密码或数据库名若含保留字符，必须按 URL 规则 percent-encode。生产 PostgreSQL
使用 `sslmode=verify-full`；新 Redis 使用 `rediss://`；不同角色使用不同 principal 和 secret。

## 6. 校验与查看

```bash
v2board-lifecycle validate --manifest /secure/private/mysql-import.json
v2board-lifecycle inspect --manifest /secure/private/mysql-import.json
```

`validate` 检查严格 JSON shape、source、target、runtime 类型和角色隔离。`inspect` 只读检查 dump
文件与其 SHA-256，并报告固定保留/丢弃边界；它不连接旧 MySQL、旧 Redis 或 Stripe，也不写 target。

通过这两个命令只证明输入静态有效，不代表数据已经导入。

## 7. 固定转换顺序

生产 importer executor 接线后，内部顺序固定为：

1. 重新验证 dump SHA-256、staging schema 和空 target；
2. 运行当前 PostgreSQL baseline；
3. 按固定 registry 转换保留行；
4. 应用 Stripe 行级规则并证明所有固定丢弃结果；
5. 校验行数、键、关系、金额、流量、token 和 sequence；
6. 保持 ClickHouse 旧事件为空，并确认新 Redis 为空；
7. 生成并验证 API/worker 配置；
8. 所有检查通过后才允许启动新版。

表名边界同样固定：旧 MySQL source 继续使用 dump 中真实的 `v2_*` 名称；新 PostgreSQL 和
ClickHouse target 从首发基线起不带该前缀。PostgreSQL 关键字冲突使用 `users`、`orders`，不是
`user`、`order`。这不是后续 rename migration，也不存在 prefixed target 兼容层。

当前仓库没有执行上述写入的 CLI 或 executor，只提供 `validate` 和只读 `inspect`。在完整路径通过
真实 MySQL 8 dump 端到端测试前，不能宣称已经可以执行生产导入，也不能用手工部分写入代替 executor。

## 8. 会保留什么

保留并转换：

- 用户、套餐、server group；
- 非 Stripe payment 配置；
- 非 Stripe 订单，包括未完成订单；
- terminal Stripe 订单的 provider-detached 业务历史；
- coupon、giftcard 与 giftcard redemption；
- invite code、commission log；
- knowledge、notice、ticket、ticket message；
- 旧 MySQL `v2_stat` 历史汇总，写入新 PostgreSQL `stat`；
- 用户 ID、password hash、永久 token、余额、MySQL 已落盘的 `u/d/transfer_enable`。

完整逐表和字段规则见[不可变契约](mysql-import-invariants.md#3-固定保留边界)。

## 9. 会丢弃什么

固定丢弃：

- 整个旧 Redis 及其中所有瞬态状态；
- MySQL `failed_jobs`；
- Stripe payment 配置和 status `0/1` Stripe 订单；
- 旧节点、route 和 credential；
- 旧 MySQL `v2_stat_user`、`v2_stat_server`；
- 旧 MySQL `v2_log`、`v2_mail_log`；
- 旧 ClickHouse history；
- 旧 `.env`、PHP/Laravel 配置、theme、前端 bundle、custom CSS/JavaScript 和 operator script。

Stripe provider 完全不联系。status `2/3/4` Stripe 订单保留时会清空 `payment_id`、`callback_no`
和派生 callback hash；用户余额保持 MySQL 原值。

## 10. 失败处理

本次导入没有需要恢复的旧数据库：旧 MySQL 从未被改变。导入失败时：

1. 不启动不完整的新系统；
2. 丢弃这次 staging 和不完整的新 target；
3. 修正 dump、数据冲突、converter、配置或基础设施；
4. 从同一个 dump 向新的空 target 再运行一次完整导入。

不要保存或续接中间导入状态。新版真正开始运行后的 backup/PITR 和故障处理是普通 native 运维，
与本次 MySQL 导入无关。

## 11. 验收

启动新版前逐项确认：

- 旧 MySQL 仍保持原状；
- dump SHA-256 与清单一致；
- 保留表和关键字段逐值验证通过；
- 固定丢弃内容没有进入 target；
- Stripe 本地行符合固定规则，且没有联系 provider；
- ClickHouse 没有旧事件，新 Redis 为空；
- 两份配置可由各自 runtime 完整解析，owner/mode 正确；
- API、worker、frontend 和数据库健康检查通过。

验收完成后，操作者再单独退役旧站并手工重建节点。旧站退役不是 importer 的隐藏阶段。
