# MySQL 一次性导入指南

本文说明如何把固定旧版的 Oracle MySQL 8 数据一次性导入尚未发布的 native 新版。数据规则见
[MySQL 一次性导入不可变契约](mysql-import-invariants.md)，数据库长期运行规则见
[PostgreSQL + ClickHouse 持久化不变量](postgresql-clickhouse-invariants.md)。

导入只有一条路径：

```text
停止旧写入
  → 导出完整 MySQL dump，计算 SHA-256 并作为受保护备份保留
  → lifecycle 在旧机直接读取原 MySQL 的只读一致性快照
  → 转换到全新专用 PostgreSQL 18 cluster
  → 以空 ClickHouse 和空 Redis 8.8 启动并建立隔离 ACL
  → 在旧机生成受限权限的新配置输出包
  → 将两份配置安全安装到新机的固定路径
  → 验证并启动新版
```

旧 MySQL 不被修改。没有 staging MySQL，也不会在新生产机安装 MySQL。dump 是完整备份和文件
完整性证据，不是 converter 输入；MySQL dump SQL 不会交给 PostgreSQL 执行。Rust converter 通过
MySQL driver 读取 typed row，逐字段校验和转换，再通过 PostgreSQL driver 使用参数化批量写入。
SQL dialect、类型、自增主键、`NULL`、布尔值、JSON、金额精度、sequence 和约束差异都在这个
明确的转换边界内处理。

## 1. 准备

准备以下资源：

- 与固定旧版 schema 匹配的 Oracle MySQL 8.0/8.4；
- 一个完整 MySQL dump；
- 旧机原 MySQL 上一个仅有目标 database `SELECT` 权限的专用导入账号；
- 一个全新专用 PostgreSQL 18 cluster；执行前除 `postgres` 外没有其他 non-template database，
  导入器将在其中创建业务 database 和分离的 migration/API/worker principals；
- 一个可访问的全新专用 ClickHouse 26.3 server/instance；清单指定的 target database 和
  principals 在执行前必须不存在；
- 一个全新、专用、整实例为空且只使用 canonical database `/0` 的 Redis 8.8；它运行
  `noeviction`、配置可写 external `aclfile`，只有关闭的 `default` user 和清单指定的非
  `default` bootstrap ACL user；
- 新 API、worker 和 frontend release；
- [`mysql-import.v1.example.json`](examples/mysql-import.v1.example.json) 的私有副本。

不要提供旧 Redis URL、Stripe credential、旧 systemd unit 或任何 MySQL 写凭据。导入器不需要它们。

## 2. 停止旧站写入

在生成最终 dump 和执行转换前，停止所有会修改旧 MySQL 的来源：

- PHP/API 和管理后台；
- scheduler、queue worker 和定时任务；
- 新订单与支付入口；
- 仍向旧站上报流量的远端节点。

这一步由操作者在导入器外完成。导入器不会控制旧服务，也不会修改旧数据库来制造写入 fence。
保持停写直到新系统验收完成，保证备份和后续读取都来自同一个已冻结的业务状态。

## 3. 导出完整备份 dump

使用 MySQL 8 `mysqldump` 导出全部业务表 schema 和全部行，并计算文件 SHA-256。不要导出 trigger、
routine 或 event。示意命令：

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

使用只读导出凭据，不执行任何 `ALTER`、`UPDATE`、`DELETE` 或 schema 修改。交给 lifecycle 工具的
dump 必须是 root-owned `0600` regular file；如何做静态加密属于操作者的文件安全措施，不扩展
导入清单格式。

dump 不会被恢复到临时 MySQL，也不会被 converter 解析。`inspected_dump_sha256` 只能证明 lifecycle
实际检查过这个备份文件，不能证明随后从独立数据库连接读取的每个字节与 dump 相同。真正用于
target 对账的是 `converted_snapshot_sha256`、保留表行数和关系校验。

## 4. 创建专用只读 source 账号

在原 MySQL 中创建一个只允许从旧机 loopback 登录、只对旧业务 database 授予 `SELECT` 的临时账号。
示意 SQL 中的 database 名、账号和密码按现场替换：

```sql
CREATE USER 'v2board_import'@'127.0.0.1'
  IDENTIFIED BY 'REPLACE_WITH_RANDOM_SECRET';
GRANT SELECT ON v2board.* TO 'v2board_import'@'127.0.0.1';
```

不要授予 `INSERT`、`UPDATE`、`DELETE`、`CREATE`、`ALTER`、`DROP`、`TRIGGER`、`EVENT`、`FILE`、
`PROCESS`、`RELOAD`、replication 或全局权限。`execute` 在读取 schema 或业务行之前建立并验证
`REPEATABLE READ`、`READ ONLY`、consistent snapshot，并读取当前账号 grants/roles；任何额外权限、
assigned/enabled role 或 `GRANT OPTION` 都会拒绝。账号权限和 transaction read-only 是两层互补保护。
converter 只能在旧机通过 `127.0.0.1` 访问该连接，并从旧机向新 PostgreSQL 发起出站连接。

`execute` 连接后、任何 target 写入前验证 server vendor/version 和固定旧 schema；14 张 imported
source 表必须全部使用 InnoDB，保证 consistent snapshot 覆盖它们。映射表的未知结构、类型、索引、engine 或关系差异会
停止转换。所有映射业务表的 source primary key 必须是正整数；任何 `id <= 0` 都在 target 发生任何
写入前使 `execute` 失败。

## 5. 填写 `mysql-import.v1`

复制示例：

```bash
cp docs/examples/mysql-import.v1.example.json /secure/private/mysql-import.json
chown root:root /secure/private/mysql-import.json /secure/private/v2board-legacy.sql
chmod 600 /secure/private/mysql-import.json
chmod 600 /secure/private/v2board-legacy.sql
```

只填写三部分：

- `source`：备份 dump path、dump SHA-256，以及使用专用只读账号且仅允许 loopback host 的
  `database_url`；
- `target`：全新专用 PostgreSQL 18 cluster、ClickHouse、一次性 Redis
  `redis_bootstrap_url` 及一个必须不存在的 `config_output_directory`；
- `runtime`：新版完整 typed 配置。

`schema_version` 固定为 `1`。保留/丢弃规则和 Stripe/Redis 处置不允许逐次选择；这些行为已经由
converter 和不可变契约固定。

URL 中的用户名、密码或数据库名若含保留字符，必须按 URL 规则 percent-encode。MySQL source URL
固定连接旧机 loopback 上的原业务 database。PostgreSQL bootstrap URL 固定连接专用空 cluster 的
`/postgres`；导入前该 cluster 不得存在其他 non-template database，导入后也不得承载其他产品
database。生产 PostgreSQL 使用 `sslmode=verify-full`。

`target.redis_bootstrap_url` 必须是专用 Redis 8.8 的
`rediss://<non-default-user>:<secret>@.../0`；它只是导入管理凭据，不会写入配置或报告。导入器要求
`default` user 关闭、ACL user 集合无旁路、`noeviction` 和 external `aclfile`，再生成随机且不同的
API/worker ACL principal。两者只允许 `PING`、`INFO memory` 与各自实际命令：API 可读写认证、
限流和节点缓存并只读 worker 指标；worker 只可读写 scheduler、reset、heartbeat、metrics、
admission 和 analytics 键，不能读取或写入 session、`TEMP_TOKEN`、step-up、OTP/TOTP 等认证键。
`CONFIG`、`DBSIZE`、`SELECT`、`FLUSH*`、`ACL` 和任意 `EVAL` 均不授予 runtime。

`config_output_directory` 位于运行 converter 的旧机，不是新机的 `/var/lib/v2board`。该目录在执行前
必须不存在，其已存在的父目录必须是 root-owned `0700` 的真实目录而非 symlink。lifecycle 工具以
root 运行；成功后创建 `0700` 输出目录，其中是 `0600` 的 `api.config.json`、
`worker.config.json` 和 `import-report.json`。

## 6. 校验、查看与执行

```bash
v2board-lifecycle validate --manifest /secure/private/mysql-import.json
v2board-lifecycle inspect --manifest /secure/private/mysql-import.json
v2board-lifecycle execute --manifest /secure/private/mysql-import.json
```

`validate` 检查严格 JSON shape、source、target、runtime 类型和角色隔离。`inspect` 只读检查 dump
文件与其 SHA-256，并报告固定保留/丢弃边界；其 machine report 明确记录
`old_mysql_contacted: false`，它不连接 source MySQL、旧 Redis 或 Stripe，也不写 target。

`execute` 连接原 MySQL 并立即进入只读一致性快照，只接受专用空 PostgreSQL 18 cluster、不存在的
PostgreSQL/ClickHouse target 和整实例为空的专用 Redis 8.8 `/0`，执行固定转换与验证，执行 Redis
`ACL SAVE`、`ACL LOAD` 后用两份新凭据重新连接做允许/拒绝探测，然后生成配置输出包。
成功报告明确记录 `old_mysql_contacted: true` 和 `old_mysql_mutated: false`。
`validate` 和 `inspect` 通过只证明静态输入有效；只有 `execute` 成功返回并生成
`status: "complete"` 的报告，才表示这次转换完整结束。这仍不代替安装配置后的启动验收。

## 7. 固定转换顺序

生产 importer 的内部顺序固定为：

1. 重新验证 dump SHA-256，在 source MySQL 建立只读一致性快照并验证 vendor、version、schema；
2. 在任何 target 写入前完成 source primary key、关系与转换前置校验，并验证所有 target 全新为空；
3. 运行当前 PostgreSQL baseline；
4. 按固定 registry 读取 typed MySQL row、转换并参数化写入保留行；
5. 应用 Stripe 行级规则并证明所有固定丢弃结果；
6. 校验行数、键、关系、金额、流量、token 和 sequence；
7. 保持 ClickHouse 旧事件为空，确认新 Redis 为空并持久化、重载、实测角色 ACL；
8. 生成并用生产 typed parser 验证 API/worker 配置，证明 bootstrap Redis secret 未进入输出；
9. 所有检查通过后才允许启动新版。

表名边界固定：旧 MySQL source 使用真实的 `v2_*` 名称；新 PostgreSQL 和 ClickHouse target 从首发
基线起不带该前缀。PostgreSQL 关键字冲突使用 `users`、`orders`，不是 `user`、`order`。这不是
后续 rename migration，也不存在 prefixed target 兼容层。

`execute` 就是这一次性写入边界；不存在手工补表、部分写入或第二条导入路径。

## 8. 安装配置输出

`execute` 成功后，在旧机核对 `import-report.json` 中的 manifest、`inspected_dump_sha256`、source
schema、converter registry、`converted_snapshot_sha256`、保留表行数和两份配置 SHA-256。
`imported_source_schema_sha256` 只绑定 14 张 imported source 表的 schema（含固定 InnoDB engine）；dump hash 是备份文件
检查证据；converted snapshot hash 绑定实际读取并最终保留的内容、关系、逐表行数和单独表示的
整表丢弃 present/absent 决策，三者不能互相冒充。

discard-only 表只校验其名称属于固定允许列表，不读取或计算总行数。报告记录固定整表丢弃决策，
而不把对数 GiB 的 `v2_stat_user`、`v2_mail_log` 等做无意义的 `COUNT(*)`。`v2_tutorial` 是允许存在
也允许不存在的旧升级残留；若存在则整表丢弃，同样不扫描其行。

通过受保护的管理通道将两份配置传输到新机，分别安装为：

```text
/var/lib/v2board/api/config.json       v2board-api:v2board-api       0600
/var/lib/v2board/worker/config.json    v2board-worker:v2board-worker 0600
```

报告中的 `redis_acl_persisted: true` 和 `redis_runtime_acl_isolated: true` 只在 ACL 已保存、重新加载并
通过真实凭据正负探测后写出；`redis_bootstrap_credential_emitted: false` 表示三份输出均未包含
bootstrap URL、username 或 password。报告只保存两份配置的 SHA-256，不保存生成的 runtime Redis secret。

固定 runtime 路径不是 manifest 中的可选项。不得在旧机将输出目录冒充新机
`/var/lib/v2board`，也不得把 API 和 worker 配置放入同一个可写目录。安装后以各自 Unix 用户运行
完整 typed parser 和启动检查；导入器不跨机代为写入这两个路径。

## 9. 会保留什么

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

## 10. 会丢弃什么

固定丢弃：

- 整个旧 Redis 及其中所有瞬态状态；
- MySQL `failed_jobs`；
- Stripe payment 配置和 status `0/1` Stripe 订单；
- 旧节点、route 和 credential；
- 旧 MySQL `v2_stat_user`、`v2_stat_server`；
- 旧 MySQL `v2_log`、`v2_mail_log`；
- 可选旧升级残留 `v2_tutorial`；
- 旧 ClickHouse history；
- 旧 `.env`、PHP/Laravel 配置、theme、前端 bundle、custom CSS/JavaScript 和 operator script。

Stripe provider 完全不联系。status `2/3/4` Stripe 订单保留时会清空 `payment_id`、`callback_no`
和派生 callback hash；用户余额保持 MySQL 原值。上述 discard-only MySQL 表都不做 row scan 或
`COUNT(*)`，因为它们不会影响 target 内容。

## 11. 失败处理

本次导入没有需要恢复的旧数据库：旧 MySQL 从未被改变。导入失败时：

1. 不启动不完整的新系统；
2. 删除不完整的新 PostgreSQL/ClickHouse/Redis target 和配置输出目录；
3. 修正备份、source 数据冲突、converter、配置或基础设施；
4. 保持旧站停写，从同一 source 向新的空 target 再运行一次完整导入。

不要保存或续接中间导入状态，也不要运行 rollback、resume、recovery 或 cleanup 工作流。新版真正
开始运行后的 PostgreSQL backup/PITR 和故障处理是普通 native 运维，与本次 MySQL 导入无关；
ClickHouse 历史可牺牲，不从本次导入或已发布 outbox 进行全量回放。

## 12. 验收

启动新版前逐项确认：

- 旧 MySQL 仍保持原状，专用 source 账号只有 `SELECT`；
- dump SHA-256 与清单一致；
- source snapshot schema、保留表和关键字段逐值验证通过；
- 固定丢弃内容没有进入 target，discard-only 表没有被无意义扫描；
- Stripe 本地行符合固定规则，且没有联系 provider；
- ClickHouse 没有旧事件，新 Redis 专用 instance 的所有 logical database 都为空且 runtime 使用 `/0`；
- Redis ACL 文件已保存并重新加载；API/worker URL 的 principal 和 secret 各不相同且都不是
  bootstrap user，worker 对所有认证键和跨 installation 键的读写/脚本访问均被拒绝；
- 两份配置可由各自 runtime 完整解析，owner/mode 正确；
- API、worker、frontend 和数据库健康检查通过。

首次 target schema 已由 `execute` 完成；不要在启动前再跑普通 PostgreSQL/ClickHouse schema job。
验收完成后，操作者删除旧机上的 lifecycle binary、manifest 和配置输出副本，撤销专用 MySQL
只读账号，撤销或轮换 PostgreSQL/ClickHouse/Redis 外部 bootstrap credential；工作 dump 按独立
受保护备份策略保留或销毁。随后再单独退役旧站并手工重建节点。上述 secret hygiene 和旧站退役
都不是 importer 的恢复状态机。
