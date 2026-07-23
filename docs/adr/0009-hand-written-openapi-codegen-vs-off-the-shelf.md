# 0009. 维持自研 OpenAPI→TypeScript+Zod 生成器,否决现成工具替换

- 状态:已取代(由 [0010](0010-hey-api-openapi-ts-component-schema-codegen.md))
- 日期:2026-07-21(2026-07-22 补充六工具实测、Rust 直出方案调研、锁定最新
  稳定版复测,以及 `@hey-api/openapi-ts` 两个硬伤的 `$resolvers` 扩展点实测
  修复)

> **2026-07-22 取代说明**:本 ADR 记录的"维持自研"结论是基于工程权衡的
> 独立评估,而非产品方向上的强制约束。项目所有者随后明确指示改用
> `@hey-api/openapi-ts`,越过了这里的工程判断——这是所有者的决策权,不是
> 本 ADR 论证被推翻。[0010](0010-hey-api-openapi-ts-component-schema-codegen.md)
> 记录实际迁移方案(组件 schema 交给 hey-api,算子注册表仍保留自研)、
> 迁移中新发现的第三个 hey-api 硬伤(`false` 字面量属性被静默丢弃)及其
> 修复方式,以及本文档下方"两个硬伤"评估如何过渡到"三个已修复硬伤"。
> 本文档保留作为六工具实测记录与被拒绝方案对比的历史依据,不再是现行决策。

## 背景(Context)

`frontend/scripts/generate-internal-api-contract.mjs`(约 896 行 + 2 个专属
测试文件共 910 行)从后端 `utoipa` 导出的 OpenAPI 3.1 规范
(`frontend/packages/api-client/openapi/internal-api.openapi.json`,25,837
行)生成两个产物:纯类型的 `@v2board/types`,以及承载**运行时** Zod 校验器
与算子执行表(`internalApiOperations`)的 `@v2board/api-client`。后者不是
可有可无的文档产物——`internal-operation.ts` 对每一次内部 API 调用的请求体、
路径/查询参数、响应体都执行 `.parse()`,是全应用生产路径上强制生效的校验层。

一次独立架构复审指出:业界已有成熟的 OpenAPI→TS/Zod 代码生成工具
(`openapi-typescript`、`orval` 等),项目选择自研而未采用,且没有任何文档
说明理由——对比项目里影响小得多的选型(如 ADR 0007 的 AES-GCM 信封加密)
都专门写了 ADR,这属于"决策留痕不一致"。

复审同时给出了两条路:要么补一份 ADR 说明自研理由,要么先做真实评估再决定。
不做验证就补一份事后合理化的 ADR 价值有限——理由很可能只是没有根据的事后
猜测。因此在起草本 ADR 前,先在 Docker 内**真实安装并运行**了两个最主流
候选工具(openapi-typescript、orval),针对当前生成器覆盖的每一项能力
逐条做了实证比对。首次定稿后追问"是否还有更好的选择",于是又补了两轮独立
评估,而不是止步于最初两个工具:

1. **扩大实测范围到六个工具**:在前两个基础上,又真实装上并跑了
   `@hey-api/openapi-ts`(含官方 Zod 插件)、`openapi-zod-client`、
   `typed-openapi`、`swagger-typescript-api`——覆盖了当前生态里叫得上号的
   主流 OpenAPI→TS/Zod 生成器。
2. **调研"不经过 OpenAPI JSON,直接从 Rust 类型生成"这条路**:因为前两轮
   暴露的核心 bug(判别联合、`additionalProperties` 语义歧义)根子在于
   OpenAPI/JSON-Schema 本身就是比 Rust 类型系统表达力更弱的中间格式——调研
   了 `specta`、`ts-rs`、`typeshare`、`zod_gen` 等直接读取 Rust 类型信息
   (而非导出的 JSON)的生成方案是否能从架构上绕开这类问题。
3. **锁定"当前真正最新稳定版"重新独立复测四个支持 Zod 输出的工具**:被追问
   "你确定测的是最新稳定版"后,逐一用 `npm view <pkg> versions` 核对每个
   工具、`typescript`、`zod` 的真实最新稳定版本号(排除 rc/beta/alpha),
   显式锁定安装,在全新 Docker 容器里独立重跑一遍,不复用前两轮的既有结论。
4. **对 `@hey-api/openapi-ts` 仅剩的两个硬伤逐一实测能否修复**:六个工具里
   表现最好的 `@hey-api/openapi-ts` 唯二两个阻断点——`additionalProperties`
   不生效、`x-v2board-max-bytes` 无扩展点——是否真的无法绕开?没有停在"文档
   里提到有 `$resolvers` 钩子"这个结论,而是直接读该工具打包后的源码
   (`dist/init-*.mjs`)确认钩子的真实调用链,写出两段真实 resolver 代码,
   跑出生成产物,再用 zod 运行时校验其行为——包括一个专门设计用来区分
   "字符数达标但 UTF-8 字节数超标"的对抗性测试用例。

## 决策(Decision)

**维持自研生成器,不替换为 openapi-typescript 或 orval。** 依据如下真实、
可复现的评估结果:

**openapi-typescript(7.13.0)**——只生成类型,零运行时代码:
- 完全没有 Zod 输出、没有 `internalApiOperations` 算子注册表,`goldens.test.ts`
  依赖的整套黄金样例校验链路无法接上,需要另配一个第三方工具才能补上运行时
  校验,而那个第三方工具同样要重新实现下面这些 v2board 专属规则。
- `additionalProperties:false` 只在**字面量对象**上给出编译期报错;经变量
  或展开传入的多余字段完全不拦截(`const b: T = payload` 编译通过)——对生产
  请求校验层是真实的安全/正确性降级,而非风格问题。
- 规范里唯一真正使用 `discriminator` 关键字的 schema——`ProblemDetails`
  (RFC 9457 通用错误信封,每个消费者都靠它判别错误类型)——生成的类型经
  `tsc` 验证是**损坏的**:`code` 字段被推断为 `never`。
- `x-v2board-max-bytes`(UTF-8 字节长度校验扩展,规范里出现 71 次)、int32
  自动 clamp 全部丢失,没有任何扩展点可以补上。

**orval(8.22.0)**——号称原生支持 Zod 输出,实测比 openapi-typescript 更差:
- 对真实、未修改的规范**直接崩溃**(`TypeError`,无法处理 `ProblemDetails`
  101 处合法的 boolean JSON-Schema 子模式 `"errors": false`),必须先在
  草稿副本里删掉这些内容才能跑通——这本身就不是一次公平的"能用"结果。
- 即便如此,`ProblemDetails` 在两种配置下都坏得比 openapi-typescript 更
  严重:`generateDiscriminatedUnion:true` 生成的 schema 在真实 zod v4 下
  **第一次 `.safeParse()` 就直接抛异常**;默认的 union 兜底模式会**拒绝所有
  合法的真实 payload**(把两个各自 `strict` 的对象用 `.and()` 相交,是已知
  的 Zod 反模式)。
- `additionalProperties` 策略只有一个全局粗粒度开关:打开则把全部 648 个
  生成对象(含本该保持开放的 114 个)统一收紧为 `strictObject`;关闭则
  101 个必须拒绝多余字段的 problem-tuple 分支全部不设防。没有按 schema
  精细判断的机制。
- `x-v2board-max-bytes`、int32 clamp、JSON-only 响应投影、真正的递归
  (`z.lazy`)处理全部缺失;唯一的自引用 schema 被暴力展开,深层字段被
  静默丢弃。

**两个工具都印证了一件事**:最初被假设为最难的能力——PATCH 语义里
"不传=保留、传 null=清空、传值=设置"的 double-Option 区分——实测**不需要
任何特殊生成逻辑**,两个工具都天然正确处理(因为规范本身只用普通的
`optional + nullable` 表达它,没有专属扩展关键字)。真正无法安全迁移的是:
运行时算子注册表的具体形状、按 schema 精确判断的 `additionalProperties`
策略、`ProblemDetails` 判别联合的正确性、`x-v2board-max-bytes`、int32
自动边界——替换意味着在新工具基础上重新实现今天生成器里的大部分 v2board
专属规则,而不是真正降低维护面。

**第二轮:另外四个主流工具,同样真实实测**

- **`@hey-api/openapi-ts`(含官方 Zod 插件,锁定版 0.99.0 复测)**——六个工具
  里表现最好的一个:对真实、未修改的规范零崩溃、零绕过跑通,生成的是真正
  匹配本仓库 zod v4.4.3 语法的运行时代码,`tsc --strict` 编译通过(0 错误),
  `double-Option`、int32/int64 clamp(格式感知,299 处 int32 与 261 处 int64
  裸字段实测正确生成边界,越界值实测被拒绝;int64 用 `z.coerce.bigint()`,
  消费方拿到的是 `BigInt` 而非 `number`,是真实的互操作细节而非 bug)、
  OA 3.1 可空类型、唯一的递归 schema(`UserOrder`,用 `z.lazy()` 正确处理)
  全部正确处理,`ProblemDetails` 判别联合和无 `discriminator` 的
  `CheckoutOutcome` 在真实 payload 上都能正确校验与收窄类型(渲染成
  `.and(z.union([...]))` 而非 `z.discriminatedUnion`,是"能用但不地道"而非
  错误)。输出锁 CI check 模式下逐字节确定性(两次生成 SHA-256 完全一致)。
  两个真实缺口——但均已实测证实可修复,细节见下方"`$resolvers` 扩展点实测
  修复"小节:① `additionalProperties:false` 默认**完全不生效**——全部 180
  个应该封闭的 schema(直接从真实 spec 解出的精确计数)都生成成允许多余
  字段静默丢弃的宽松对象,读打包源码(`dist/init-D6Y8JFUS.mjs`,对应
  `src/plugins/zod/v4/toAst/object.ts`)确认根因:只要 schema 带具名
  `properties`,内置的 `additionalPropertiesNode` 就直接提前返回
  `undefined`,压根不读 `additionalProperties` 的值——用官方文档过的
  `$resolvers.object` 钩子写了一段约 15 行的自定义 resolver,生成产物、
  zod 运行时校验均实测通过。② `x-v2board-max-bytes`(71 处)默认零校验,
  同样用 `$resolvers.string` 钩子补上 `Buffer.byteLength` 字节长度
  `.refine()`,专门构造了"字符数达标、UTF-8 字节数超标"的对抗性测试用例
  (200 个中文字符,字符数 200<512 但字节数 600>512)实测被正确拒绝。另外
  安装本身有一个未在官方文档标注的陷阱:该工具 `peerDependencies` 里
  `typescript` 范围写的是 `>=5.5.3 || >=6.0.0`,没有真实上限,一次
  未显式锁定 typescript 版本的 `npm install` 会被解析到 typescript 7.0.2
  (新的原生/Go 重写分支),而该工具打包后的代码在 7.x 下直接崩溃
  (`TypeError: Cannot read properties of undefined (reading 'AnyKeyword')`,
  因为它运行时读取的内部 AST 常量 `ts.SyntaxKind.AnyKeyword` 在 7.x 重写后
  的内部结构里不存在了)——工具自己的 `devDependencies` 也只钉到
  `typescript@6.0.3`,说明这从未在 7.x 下被真实测试过。这不是本仓库特有的
  坑(本仓库自己的 `frontend/pnpm-workspace.yaml` 已经把 `typescript` 别名
  锁定到 `npm:@typescript/typescript6@^6.0.2`,不会触发这条 auto-resolve
  路径),但任何裸装 `@hey-api/openapi-ts` 的新项目/CI 都会撞上。响应头
  建模(302 快速登录的 `Location`、CSV 导出的 `Content-Disposition`)确实
  不生成,但查证本仓库前端代码后发现**不是真实功能性缺口**——302 由浏览器
  原生处理跳转,前端 JS 从未读取过这个响应头;CSV 文件名是前端自己拼的
  (`frontend/apps/admin/src/pages/users/index.tsx` 的
  `downloadGeneratedUserCsv`),同样不读 `Content-Disposition`。真正没有
  对应机制、且这次复测里没有找到可行 workaround 的,是
  `internalApiOperations` 风格的运行时算子注册表(不是 schema 校验问题,
  是这整套"把每个 operation 映射成可执行调用"的运行时抽象在 hey-api 里没有
  对应产物)和 JSON-only 响应投影。
- **`openapi-zod-client`(锁定版 1.18.3 复测)**——比 `@hey-api/openapi-ts`
  全面更差,且默认配置比想象中更能"骗过"人:默认(不加任何 flag)不会崩溃,
  因为它默认只为每个 operation 的**成功响应**生成类型,直接整体丢弃所有
  错误响应 schema——`ProblemDetails` 判别联合从未被处理,不是修好了,是
  压根没碰。一旦用 `--export-schemas` 强制它真正处理 `ProblemDetails`,才
  和 orval 同款报错崩溃(同一处 boolean 子模式);绕过之后又挖出一个此前
  未提及的独立 bug——完全不支持 JSON-Schema 的 `const` 关键字(`enum` 没事,
  `const` 一律退化成 `z.unknown()`/裸类型),这才是判别字段丢失的真根因,
  不只是崩溃的副作用;由此生成的 `z.discriminatedUnion` 实测**第一次
  `.safeParse()` 就抛异常**(`Invalid discriminated union option`)。输出是
  **Zod v3 语法**,对本仓库钉住的 zod v4.4.3 编译报错——精确重新计数是默认
  172-endpoint 产物里 14 处 TS2554(共 16 处 tsc 错误),强制导出全部 schema
  后 29 处 TS2554(共 34 处);工具自身运行时依赖 `@zodios/core@10.9.6` 还
  硬性钉死 `peer zod@"^3.x"`,对本仓库钉住的 `zod@^4.4.3` 跑 `npm install`
  直接 ERESOLVE 失败,要 `--legacy-peer-deps` 才能装上——不是"语法不兼容"
  这么轻,是包管理层面就与本仓库的 zod 版本冲突。唯一的递归 schema
  `UserOrder` 不是"处理得不好",是**加载时直接抛 `ReferenceError` 崩溃**
  (默认配置下就会,不需要特殊 flag)。`additionalProperties` 同样只有全局
  粗粒度开关,且实测该开关本身有 bug:打开后对本该保持开放的 schema 会生成
  自相矛盾的 `.strict().passthrough()` 链,`.passthrough()` 排在后面在 zod
  运行时胜出,导致开关对这些 schema 完全不生效。
- **`typed-openapi`(锁定版 2.2.7 复测)**——同样在 `ProblemDetails` 的
  boolean 子模式上无条件崩溃,绕过之后产物**编译不通过**(精确重新计数
  332 处真实 `tsc` 报错,和上一轮结论吻合,主因是 326 处 `z.record()`
  单参数调用不符合 zod v4 的两参数签名),而且比"静默丢失 4 个操作"描述的
  更严重:那 4 个操作(`AdminServersCreate` 等)**不是被丢弃**,而是被生成
  在一个带 `_properties_responses` 等后缀的错误名字下,变成没人引用的死
  代码,而调度表 `EndpointByMethod` 仍然引用它们本该有的正确名字——这个
  名字在文件里从未被声明过。用 `tsx`(esbuild,跳过类型检查)实测验证:这
  导致 **导入这个生成文件里的任何一个符号都会在模块求值阶段直接抛
  `ReferenceError`**——因为 ESM 必须先完整求值模块顶层代码(含调度表字面量)
  才能让任何 export 变成可用,所以受影响的不只是那 4 个操作,是全部 158 个
  operation、207 个 schema 一起完全不可用,直到有人手工 patch。并且完全不
  支持 JSON-Schema 的 `const` 关键字,导致 `ProblemDetails` 判别字段全部
  退化成 `z.unknown()`——判别能力被直接摧毁,实测一个本该被拒绝的非法
  payload(带禁止字段)被错误放行。
- **`swagger-typescript-api`**——和 openapi-typescript 一样只生成类型、零
  运行时代码,这一条直接不合格;更严重的是,**默认配置**下 `ProblemDetails`
  的判别字段会坍缩成 TypeScript 的底类型 `never`(用非默认的
  `--enum-style=union` 才能避开),六个工具里默认配置表现最差的一次。

**`$resolvers` 扩展点实测修复:`@hey-api/openapi-ts` 仅剩两个硬伤是否真的
堵死**

六个工具里表现最好的 `@hey-api/openapi-ts` 剩下两个硬伤——
`additionalProperties:false` 不生效、`x-v2board-max-bytes` 无扩展点——是否
真的没法绕开,没有停留在"官方文档提到有 resolver 钩子"这句话上,而是直接
读该工具打包后的源码确认真实调用链、写出可运行的自定义 resolver、跑出
生成产物,再用 zod 运行时校验其行为是否正确。

- **根因**(直接读 `dist/init-D6Y8JFUS.mjs`,对应源码路径
  `src/plugins/zod/v4/toAst/object.ts`):默认的 `additionalPropertiesNode`
  函数只要 schema 带具名 `properties` 就直接提前 `return undefined`,根本
  不检查 `additionalProperties` 的值——只有零具名字段的纯 map 型对象才会
  走到 `additionalProperties` 驱动的代码路径。IR 层还有一个隐藏细节:原始
  spec 里的布尔值 `additionalProperties: false` 进入内部 IR 后会被规整成
  `{"type": "never"}`(不再是布尔值),自定义 resolver 如果直接判断
  `=== false` 会静默失效——这个 IR 规整行为没有写在公开文档里,是调试时加
  `console.error` 打印 `ctx.schema` 才发现的,踩了一次才摸清楚。
- **`$resolvers.object` 补丁**(约 10 行,修复 additionalProperties):
  ```js
  object(ctx) {
    const { schema, plugin, $ } = ctx;
    const ap = schema.additionalProperties;
    const closed = ap === false || (ap && typeof ap === 'object' && ap.type === 'never');
    let node = $(plugin.imports.z).attr('object').call(/* ...shape... */);
    if (closed) node = node.attr('strict').call();
    return node;
  }
  ```
  实测生成产物变成 `z.object({...}).strict()`,zod 运行时 `.safeParse()`
  对多传的未声明字段正确返回 `success:false`。
- **`$resolvers.string` 补丁**(补 `x-v2board-max-bytes` 字节长度校验):
  ```js
  string(ctx) {
    const { schema, plugin, $ } = ctx;
    let node = $(plugin.imports.z).attr('string').call();
    const maxBytes = schema['x-v2board-max-bytes'];
    if (typeof maxBytes === 'number') {
      const predicate = $.func((f) => {
        f.param('val');
        f.do($.binary(
          $.attr($.id('Buffer'), 'byteLength').call($.id('val'), $.literal('utf8')),
          '<=', $.literal(maxBytes)
        ).return());
      });
      node = node.attr('refine').call(predicate, /* ...message... */);
    }
    return node;
  }
  ```
  生成产物是
  `z.string().max(512).refine(val => Buffer.byteLength(val, 'utf8') <= 512, {...})`。
  专门构造了一组对抗性 zod 运行时用例区分"字符数"和"字节数"这两个容易
  混淆的维度:200 个中文字符(字符数 200,远低于 512 字符上限;UTF-8 字节数
  600,超过 512 字节上限)——**正确拒绝**,且报错正是新加的字节校验,不是
  字符数校验;170 个中文字符(510 字节,两个上限都满足)——正确通过;600
  个 ASCII 字符(字符数、字节数都超限)——正确拒绝,两条错误同时报出。这
  正是当初 Rust 后端要发明 `x-v2board-max-bytes` 而不是直接用标准
  `maxLength` 的原因:OpenAPI/JSON-Schema 标准的 `maxLength` 定义死了是数
  Unicode 字符数,没有任何标准关键字能表达字节长度约束,而 Rust 侧的真实
  限制(`backend/rust/crates/api/src/request_params.rs`、
  `backend/rust/crates/api/src/admin.rs` 的 `mail_idempotency_key`)是按
  `.len()`(字节)算的——两者不一致时,字符数校验通过但字节数超限的输入会
  被前端放行、被后端拒绝,前端校验形同虚设。
- **两个补丁都用的是 hey-api 官方文档过的、有类型定义的 `$resolvers`
  扩展点**(`Plugin.Resolvers<T>`,zod 插件的 `ZodResolvers` 类型),不是
  私有 API hack,但有一个值得记录的前瞻风险:`$resolvers.object` 补丁依赖
  上面提到的 IR 规整行为(`false` → `{type:"never"}`)——这是内部实现细节,
  不是 `$resolvers` 类型契约的一部分,hey-api 未来版本升级 IR 表示方式时
  可能悄悄失效而不报错(校验静默变回不生效,而不是编译报错),需要一条
  针对这两个 resolver 的回归测试,而不是装完就当作永久有效。
- **结论**:这两个此前被记成"硬伤、没有配置项能打开"的问题,实际上是
  "可以精确修复、且已经实测验证正确"的——真正的成本不是"技术上无法绕开",
  而是需要团队自己写、自己测、自己长期维护这两段(以及未来可能更多的)
  自定义 resolver 代码,并且要接受它耦合到 hey-api 未公开保证的内部行为
  这一风险。这比"完全没有出路"轻,但也不是零成本——采用 hey-api 仍然意味着
  把"生成器行为正确性"的一部分维护责任,从"自己实现全部逻辑"换成"自己实现
  两个 resolver + 追踪上游内部实现变化",而 `internalApiOperations` 算子
  注册表这类运行时抽象依然完全没有对应产物,要么保留自研的这一层、要么
  另外新写。

**调研:跳过 OpenAPI JSON,直接从 Rust 类型生成**

前两轮暴露的核心 bug——判别联合识别错误、`additionalProperties` 语义
歧义——根源是 OpenAPI/JSON-Schema 本身表达力弱于 Rust 类型系统(Rust 的
带标签枚举导出成 JSON-Schema 只能退化成 `oneOf` + 可选的 `discriminator`,
这个退化本身就是问题的根源)。调研了三个**直接读取 Rust 类型信息**(不
经过 OpenAPI JSON 中转)的成熟 Rust crate——`specta`(Tauri 生态,617
星)、`ts-rs`(1.8k 星,判别联合支持最成熟)、`typeshare`(1Password
维护,3k 星)——以及唯一一个真正有可用 Zod 输出的小众 crate `zod_gen`。
结论:**这条路能从架构上根治判别联合的歧义**(因为直接读 Rust 的
`serde` tag 信息,不再经过有损的 JSON-Schema 往返),但只解决了"类型"这
一半——`specta`/`ts-rs`/`typeshare` 三个成熟工具里没有一个有生产可用的
Zod 输出(`specta` 官方的 `specta-zod` 伴生 crate 才发布两个月,自己文档
都写"部分/计划中";`ts-rs`/`typeshare` 压根没有 Zod 故事);`zod_gen` 虽然
判别联合处理得很好,但 16 颗星、月下载量约 122,且明确不支持
`deny_unknown_fields`,达不到本仓库对 `additionalProperties` 的强制要求。
而且这条路**不能替代**现有的 utoipa OpenAPI 导出(该产物仍要留给 API 文档
等其他消费方),会变成第二条独立维护的生成流水线,每个契约类型要挂两套
派生宏/注解,还需要新增一个 golden/parity 测试防止两边语义漂移——净效果
是"自研代码量减少但不会归零,且多了一条新流水线要维护同步",不是单纯的
减法。

**明确拒绝的替代方案:**

- **openapi-typescript + 第三方 Zod 配套**(如 `openapi-zod-client`、
  `ts-to-zod`)—— 实测这类配套仍需重新实现 `additionalProperties` 精确
  策略、`x-v2board-max-bytes`、算子注册表,且 `ProblemDetails` 的 `never`
  类型 bug 需要手工 patch,维护面未见真实下降。
- **orval(zod 模式)**—— 实测无法处理真实规范中的 boolean 子模式,且在
  最关键的判别联合场景上产生运行时崩溃与拒绝合法数据的更严重回归。
- **`openapi-zod-client`、`typed-openapi`、`swagger-typescript-api`**——
  逐一实测后否决,具体原因见上(Zod v3 不兼容本仓库钉住的 v4、编译不通过、
  静默丢操作、判别字段被摧毁、零运行时校验)。
- **`@hey-api/openapi-ts`**——六个工具里最强,且经二次锁定版本复测确认结论
  稳定。原先记录的"`additionalProperties` 强制校验、响应头建模"两个硬伤,
  经实测已分别查明:前者用官方 `$resolvers.object` 钩子可精确修复(已写出
  可运行补丁并验证)、后者查证本仓库前端代码后发现不是真实功能性缺口(302
  由浏览器原生处理,CSV 文件名前端自己拼,均不读那个响应头)。现在仍然
  否决直接替换的真正理由,是`internalApiOperations` 风格的运行时算子
  注册表在 hey-api 里没有对应产物(不是 schema 校验问题,是整套"契约→
  可执行调用"的运行时抽象要么保留自研这一层要么另起炉灶)、现有
  114/101 条覆盖测试与黄金样例链路需要重新对接,以及采用 `$resolvers`
  意味着要自己长期维护并回归测试两段自定义 resolver、并承担它耦合到
  hey-api 未公开保证的内部 IR 行为这一前瞻风险。这些是真实的工程量与
  维护责任转移,不是"技术上无法绕开"——若未来重新评估替换,这仍然是
  应该优先考虑、且已经把两个原以为堵死的点验证清楚的候选基础,而不是从零
  再测一遍。
- **跳过 OpenAPI JSON、直接从 Rust 类型生成**(`specta`/`ts-rs`/
  `typeshare` + 自建 Zod 层)——架构上更干净,但今天没有成熟的 Zod 产出
  方案,且会新增一条要手动保持同步的平行生成流水线,净收益不确定,不是
  "不计成本"以外场景下现在就该做的事。
- **不做验证直接补一份事后合理化 ADR**—— 被否决,因为这样的"理由"和
  复审本身批评的问题(决策没有留痕依据)是同一类错误;本决策改为先做三轮
  真实工具安装、运行、实证比对(共六个工具 + 一次架构调研),再落笔。

## 后果(Consequences)

- 得到:自研生成器的存在有了具体、可复核的实证依据,而非猜测或工具刻板
  印象;六个候选工具的具体失败模式(`ProblemDetails` never 类型、orval/
  openapi-zod-client/typed-openapi 共同的 boolean-subschema 崩溃、
  additionalProperties 全局开关粗粒度或完全不生效、Zod v3/v4 不兼容、
  递归 schema 加载崩溃)已记录在案,避免团队未来重复同一轮评估。
- 得到:识别出一个具体的"若未来重新评估,从这里开始"候选——
  `@hey-api/openapi-ts`——并把它原先记录的两个阻断点逐一查清:
  `additionalProperties:false` 不生效已用 `$resolvers.object` 钩子写出、
  验证过一段可运行修复;`x-v2board-max-bytes` 同样用 `$resolvers.string`
  钩子验证过修复(专门测过"字符数达标、字节数超标"的对抗性用例);响应头
  完全不建模经查证并非本仓库真实消费的功能,不是障碍。真正剩下的是
  `internalApiOperations` 算子注册表这类运行时抽象没有对应产物,以及
  采用 `$resolvers` 需要自行长期维护两段自定义 resolver、承担其耦合到
  hey-api 未公开 IR 行为的前瞻风险——是精确的工程量与维护责任清单,不是
  一句笼统的"现成工具不够好"。
- 得到:确认"跳过 OpenAPI JSON、直接从 Rust 类型生成"这条架构上更干净的
  路径目前不成熟——`specta`/`ts-rs`/`typeshare` 没有一个有生产级 Zod 输出,
  且该路径不能替代现有 utoipa OpenAPI 导出,只会新增一条要手动保持同步的
  平行流水线,净收益不确定。
- 代价:继续独自维护约 900 + 910 行生成器代码与其专属测试——这是真实、
  持续的单人维护负担,是 finding #2(全仓库单人维护过载)整体问题的一个
  具体实例,并未被本决策解决,只是确认"现在没有更好的现成替代"。
- 代价:本决策不是一次性豁免,且触发条件比最初设想的更容易达成——
  `additionalProperties` 与 `x-v2board-max-bytes` 已经证明可以用
  `$resolvers` 精确修复,不再需要等 hey-api 官方补上;真正悬而未决的是
  `internalApiOperations` 算子注册表这层运行时抽象要不要保留、要不要
  投入工程量把 hey-api + 两段自定义 resolver + 一层算子注册表拼接成完整
  替代品。若团队评估这份工程量可接受,或 `specta-zod`/同类 Rust 原生 Zod
  输出方案成熟到生产可用,应重新触发同等深度的实证评估(含真实实现算子
  注册表这一层,而不是止步于 schema 校验层面),而不是想当然援引本 ADR
  继续维持现状。

## 证据

- [`../../frontend/scripts/generate-internal-api-contract.mjs`](../../frontend/scripts/generate-internal-api-contract.mjs)
  —— 自研生成器本体:`additionalProperties` 显式策略校验
  (`assertExplicitObjectPolicies`)、`x-v2board-max-bytes`、int32 clamp、
  Tarjan SCC 递归检测均在此实现。
- [`../../frontend/packages/api-client/src/internal-operation.ts`](../../frontend/packages/api-client/src/internal-operation.ts)
  —— 运行时对每次请求/响应调用 `.parse()`,是 Zod 输出必须存在的直接原因。
- [`../../frontend/packages/api-client/src/goldens.test.ts`](../../frontend/packages/api-client/src/goldens.test.ts)、
  [`../../backend/rust/crates/api/src/golden_wire.rs`](../../backend/rust/crates/api/src/golden_wire.rs)
  —— 跨语言黄金样例校验链路,任何替换方案都必须能接上。
  [`../../frontend/scripts/internal-api-contract-coverage.test.mjs`](../../frontend/scripts/internal-api-contract-coverage.test.mjs)
  —— 钉住 114 个"显式开放"对象与 101 个 RFC 9457 problem 分支的精确策略。
- [`../api-dialect.md`](../api-dialect.md) §4.4(double-Option 语义)、
  §9.3(结账判别联合)—— 本 ADR 引用的两处最关键 schema 场景的规范定义。
- 六个工具(`openapi-typescript`、`orval`、`@hey-api/openapi-ts`、
  `openapi-zod-client`、`typed-openapi`、`swagger-typescript-api`)与四个
  直接从 Rust 类型生成的方案(`specta`/`specta-zod`、`ts-rs`、`typeshare`、
  `zod_gen`)均针对本仓库当前真实的
  [`openapi.json`](../../frontend/scripts/generate-internal-api-contract.mjs)
  输出与 `zod@4.4.3`(见 `frontend/pnpm-workspace.yaml` catalog)在一次性
  Docker 容器中安装、运行、编译实测,未依赖工具文档或既有印象判断。
- 支持 Zod 输出的四个工具(`@hey-api/openapi-ts`、`orval`、
  `openapi-zod-client`、`typed-openapi`)额外做了一轮独立复测:先用
  `npm view <pkg> versions` 核对每个工具、`typescript`、`zod` 当前真正
  的最新稳定版(排除 rc/beta/alpha),显式锁定版本号,在全新 Docker
  容器里重新安装、运行、编译,不复用前一轮已有结论——用于回应"你确定测的
  是最新稳定版"的追问,发现并记录了一个此前遗漏的真实陷阱(`@hey-api/
  openapi-ts@0.99.0` 的 `typescript` peerDependencies 范围没有真实上限,
  裸装会被 npm 解析到 typescript 7.0.2 并直接崩溃,需要显式锁定
  typescript 版本;本仓库自身的 `typescript` 别名锁定不受影响)。
- `@hey-api/openapi-ts` 的 `additionalProperties`、`x-v2board-max-bytes`
  两个阻断点额外做了 `$resolvers` 扩展点实测:直接读该工具打包后的源码
  (`@hey-api/openapi-ts@0.99.0` 的 `dist/init-D6Y8JFUS.mjs`,对应源码路径
  `src/plugins/zod/v4/toAst/{object,string}.ts`)确认钩子真实调用链与
  IR 规整行为,写出两段可运行的自定义 resolver,生成产物并用 zod 运行时
  校验(含专门构造的"字符数达标、UTF-8 字节数超标"对抗性用例),结果均
  记录在上方"`$resolvers` 扩展点实测修复"小节。
