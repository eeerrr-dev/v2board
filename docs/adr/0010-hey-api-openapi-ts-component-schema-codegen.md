# 0010. 组件 schema 生成改用 @hey-api/openapi-ts,运行时算子注册表继续自研

- 状态:已采纳
- 日期:2026-07-22

## 背景(Context)

[ADR 0009](0009-hand-written-openapi-codegen-vs-off-the-shelf.md) 基于三轮
六工具实测,得出"维持自研生成器"的工程结论:`@hey-api/openapi-ts` 是六个
工具里表现最好的一个,原先记录的两个阻断点(`additionalProperties` 不
生效、`x-v2board-max-bytes` 无扩展点)均已用 `$resolvers` 钩子实测修复,
但仍以"`internalApiOperations` 风格的运行时算子注册表在 hey-api 里没有
对应产物"以及"采用 `$resolvers` 意味着长期维护自定义 resolver 并耦合到
hey-api 未公开 IR 行为"为由,否决了直接替换。

项目所有者随后明确指示改用 hey-api。这是所有者对工程权衡的决策权,不是
对 ADR 0009 论证或实测结果的推翻——ADR 0009 记录的每一条实测缺口依然
真实存在,只是不再构成否决理由。本 ADR 记录：在"必须换成 hey-api"这个
前提下,ADR 0009 已经识别出的缺口(尤其是算子注册表)如何被实际处理,
以及迁移过程中新发现的、ADR 0009 两轮实测都未触发的第三个真实 hey-api
硬伤。

## 决策(Decision)

**组件 schema(`components.schemas` 全部 207 个)的类型与 Zod v4 运行时
校验器改由 `@hey-api/openapi-ts@0.99.0`(锁定版)生成;运行时算子注册表
(`internalApiOperations`)、参数/请求头/查询校验、路径模板展开等
ADR 0009 已确认 hey-api 无等价产物的部分,继续保留自研实现不变。** 这是
一次混合架构迁移,不是整体替换——ADR 0009 关于算子注册表这一点的实测
结论被本决策直接采纳,而不是重新评估。

- `frontend/scripts/generate-internal-api-contract.mjs` 调用 hey-api 的
  Node API(`createClient`),产物写入
  `frontend/packages/types/src/generated/hey-api/{types.gen.ts,zod.gen.ts}`;
  脚本内保留的规范校验(`assertExplicitObjectPolicies`)、算子注册表渲染
  函数(`typeExpression`/`zodExpression`/`operations()` 等)与 ADR 0009
  草稿里写出的三段 `$resolvers` 补丁(见下)均未删除。
- **完整向后兼容别名层**:`packages/types/src/generated/internal-api.ts`
  为全部 207 个 schema 生成一行别名(`export type InternalApi<Name> =
  <heyApi 类型名>` 与 `export const internalApi<name>Schema =
  <heyApi zod 常量名>`);`packages/api-client/src/generated/internal-api.ts`
  从 `@v2board/types` 导入这些常量供本地 `internalApiOperations` 使用,
  并原样 re-export,历史消费方(直接从
  `packages/api-client/src/generated/internal-api` 导入 schema 常量的代码)
  不需要改一行导入路径。经 diff 核实:迁移前后 207 个类型名与 207 个
  schema 常量名逐一比对完全一致,零消费方可见的破坏性变更。
- ADR 0009 草稿里实测过的两段 `$resolvers` 补丁原样进入生产:
  `heyApiStringResolver` 补 `x-v2board-max-bytes` 的 UTF-8 字节长度
  `.refine()`;`heyApiNumberResolver` 把 `int64`/`uint64` 格式从 hey-api
  默认的 `z.coerce.bigint()` 改回 `z.number()`,匹配本仓库现有的
  `number` 线上约定(避免消费方从 `number` 静默变成 `BigInt`)。
- ADR 0009 记录的 `additionalProperties:false` 补丁(`.strict()`)实测
  时只覆盖了"零具名属性"的场景;真正接入生产规范后额外发现一个 ADR 0009
  两轮实测都没有触发的次生 bug——**具名属性 + `additionalProperties:
  true` 的开放对象**(如 `ProblemDetails` 本体)hey-api 默认渲染成不带
  catchall 的 `z.object(...)`,Zod 默认对未声明字段直接静默丢弃,与生成
  的 `[key: string]: unknown` TypeScript 类型承诺矛盾。这是通过
  `frontend-build` 的完整 `tsc --noEmit`(而非仅覆盖测试)才捕获到的真实
  回归,补丁是让 `heyApiObjectResolver` 对这类 schema 追加
  `.catchall(z.unknown())`。
- **本 ADR 新增的第三个真实 hey-api 硬伤,ADR 0009 的两轮实测均未触发**:
  hey-api 的 schema/IR 归一化会静默丢弃任何取值为字面量布尔 `false` 的具名
  属性(JSON Schema"此键禁止出现"的标准写法)——发生在 `$resolvers` 被
  调用**之前**,resolver 层面拿到的 `schema.properties` 里这个键已经不
  存在,任何 resolver 补丁都无法对它做出反应。本仓库规范里
  `ProblemDetails` 约 100 个非 `validation_failed` 的判别分支正是用
  `"errors": false` 表达"此错误码禁止携带 `errors` 字段"这条 RFC 9457
  规则——被静默丢弃后,该字段实际上从"禁止"退化成"未声明但被 catchall
  放行的任意值",是真实的校验层退化(`internal-api-contract.test.ts` 里
  已有的对抗性测试用例会捕获到,但如果只跑覆盖测试而不跑 api-client 的
  完整 vitest 套件,这个回归会被漏掉)。修复不能停留在 resolver 层:改为
  在把规范喂给 hey-api **之前**,对一份克隆规范做预处理——把每个
  `false` 取值的属性从 `properties` 里摘掉,属性名记录进一个
  `x-v2board-forbidden-properties` 供应商扩展数组(与
  `x-v2board-max-bytes` 一样,是 hey-api 会原样透传的未知字段),
  `heyApiObjectResolver` 读取这个数组,用 Zod v4 的 `.extend({ name:
  z.never().optional() })` 把每个字段重新接回 schema、再叠加
  `.catchall()`,让"未声明字段放行,已声明的禁止字段拒绝"两条规则同时
  成立。原始 `spec`(供保留的 `typeExpression`/`zodExpression` 等函数
  使用)不受影响,只有喂给 hey-api 的克隆规范被修改。经隔离的合成 spec
  测试与 `internal-api-contract.test.ts` 的真实 RFC 9457 用例双重验证。

## 后果(Consequences)

- 得到:生成器脚本里处理组件级 allOf 合并
  (`resolveAllOfObjectMember`/`mergedAllOfObject`)、组件级对象渲染
  (`objectTypeExpression`/`zodObjectExpression`)、legacy `nullable: true`
  (`withNullable`)、以及 `typeExpression`/`zodExpression` 内的
  oneOf/anyOf/allOf/OpenAPI-3.1 `type` 数组联合/`const` 分支——已实际删除
  (不是仅仅"不再触发"),改由 hey-api 生成。删除依据:对当前 26k 行规范的
  穷举遍历确认这些分支在 operation 级别(这些函数如今唯一能看到的数据)
  出现次数均为零;保留的分支处替换为断言,一旦未来规范在 operation 级别
  真的出现这些形状会显式报错,而不是静默残留不可达代码。保留的是规范校验、
  名称映射、三段 resolver/预处理补丁,以及 ADR 0009 已确认必须自研的
  算子注册表逻辑——脚本行数因此实际减少。
- 得到:`extractForbiddenProperties` 预处理改用 hey-api 自带、文档化的
  `parser.patch.schemas` 钩子触发(而不是脚本自己遍历
  `components.schemas` 后写入临时文件再喂给 `createClient`)——消除了
  `mkdtemp`/`writeFile`/`rm` 的临时规范文件生命周期,`input` 直接传入
  一份 `structuredClone(spec)`。递归摘除逻辑本身不变,只是触发方式换成
  hey-api 官方支持的扩展点;同时补上了递归遍历里遗漏的 `propertyNames`
  子树(此前该分支若嵌套了字面量 `false` 属性会被静默漏检,当前规范里
  未发生但属于潜在缺口)。
- 得到:新增一条针对具名属性 + `additionalProperties: false` 的闭合对象
  的显式 `.strict()`/多余字段拒绝回归测试
  (`internal-api-contract.test.ts`)——此前只有开放对象的
  `.catchall()` 计数断言,闭合对象的拒绝行为完全没有运行时回归覆盖。
- 得到:`frontend/packages/types/src/generated/hey-api/` 现已提交进版本库
  (与同目录 `internal-api.ts` 的既有约定一致),修复了一个此前存在的、
  未被察觉的 CI 隐患——该目录此前既未提交也未被 `.gitignore` 排除,
  `make api-contract-check`/CI 在全新 checkout 上会因文件不存在而
  `ENOENT` 失败,只是本地开发者已生成过一次所以从未在本机触发。
- 上述四项均为迁移落地后、针对"这次迁移是否足够干净彻底"的一次自我
  对抗性复核(多角度并行审计 + 独立验证每条结论)发现并当场修复,不是
  原始迁移提交的一部分。
- 得到:零消费方可见的破坏性变更。`make api-contract-generate`、
  `make api-contract-check`(含 `--check` 临时目录路径的幂等性)、
  `internal-api-contract-coverage.test.mjs` 全部 10 项断言、
  `@v2board/api-client` 211 个 vitest 用例、`@v2board/admin` 400 个
  vitest 用例、`@v2board/user` 456 个 vitest 用例(唯一失败项是一个与本次
  迁移无关的既有 `dayjs` optimizeDeps 漂移,`apps/user` 目录本身零未提交
  改动可证),以及两个前端应用的完整 `tsc --noEmit`,均为绿色。
- 代价(ADR 0009 已预警的风险,现在从假设变成生产事实):自定义补丁从两段
  变成三段,其中新增的"禁止属性"补丁比原先两段耦合更深——它依赖的不是
  `$resolvers` 类型契约,而是 hey-api 在 resolver 被调用**之前**对
  `false` 字面量属性的静默丢弃这一未公开内部行为。hey-api 未来版本升级
  IR 处理方式时,三段补丁中的任意一段都可能悄悄失效而不报编译错误,
  必须持续依赖 `internal-api-contract-coverage.test.mjs` 与
  `internal-api-contract.test.ts` 的现有断言作为存活回归门禁,而不是
  一次性验证。
- 代价:`internalApiOperations`(158 个 operation 的参数/请求头/查询
  校验、路径模板展开)完全保留自研——ADR 0009 关于 hey-api 无等价运行时
  抽象的结论被直接采纳而非重新论证;按 ADR 0009 自己的行数统计
  (约 900+910 行),本次迁移影响的是其中的组件-schema 渲染部分,算子
  注册表这一半基本不受影响。
- 代价:`ProblemDetails`、`CheckoutOutcome` 两处判别联合仍渲染成
  `z.union([...]).and(...)` 而非地道的 `z.discriminatedUnion`——ADR 0009
  已记录、本次迁移未改变的已知风格缺口。
- 未解决:本决策不消除 ADR 0009 point 出的单人维护负担(finding #2 的
  一个具体实例),只是把负担的形状从"自己实现全部 schema 渲染逻辑"换成
  "自己实现并长期维护三段耦合 hey-api 内部 IR 行为的补丁,外加算子
  注册表"——维护责任转移,不是消失。

## 证据

- [`../../frontend/scripts/generate-internal-api-contract.mjs`](../../frontend/scripts/generate-internal-api-contract.mjs)
  —— `heyApiObjectResolver`/`heyApiNumberResolver`/`heyApiStringResolver`
  三段补丁、`extractForbiddenProperties` 规范预处理、`createClient` 调用、
  保留的算子注册表渲染逻辑与向后兼容别名层(`renderTypes`/`renderRuntime`)
  均在此文件。
- [`../../frontend/packages/types/src/generated/hey-api/`](../../frontend/packages/types/src/generated/hey-api/)
  —— hey-api 直接生成的 `types.gen.ts`/`zod.gen.ts`。
- [`../../frontend/packages/types/src/generated/internal-api.ts`](../../frontend/packages/types/src/generated/internal-api.ts)、
  [`../../frontend/packages/api-client/src/generated/internal-api.ts`](../../frontend/packages/api-client/src/generated/internal-api.ts)
  —— 向后兼容别名层与保留的算子注册表。
- [`../../frontend/scripts/internal-api-contract-coverage.test.mjs`](../../frontend/scripts/internal-api-contract-coverage.test.mjs)、
  [`../../frontend/packages/api-client/src/internal-api-contract.test.ts`](../../frontend/packages/api-client/src/internal-api-contract.test.ts)
  —— 三段补丁(含"禁止属性"预处理)的存活回归门禁,含 RFC 9457
  判别分支的对抗性用例。
- [`../../Makefile`](../../Makefile) 的 `api-contract-generate`/
  `api-contract-check` 目标 —— 生成与漂移检查的 Docker 化入口。
- [0009](0009-hand-written-openapi-codegen-vs-off-the-shelf.md) —— 三轮
  六工具实测记录与被拒绝方案对比,本 ADR 的前置依据,现由本文档取代其
  "维持自研"结论,但不否定其实测数据。
