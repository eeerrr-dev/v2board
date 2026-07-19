import type { Plan } from '@v2board/types';
import type { FormCtx } from '../schema';
import { Section, SelectRow, SwitchRow, TextRow, TextareaRow } from '../rows';
import { parseBackendNumber, selectInteger } from '../values';

export function SiteSection({ ctx, plans }: { ctx: FormCtx; plans: Plan[] }) {
  const tryOutOff = String(ctx.get('site', 'try_out_plan_id') ?? 0) === '0';
  return (
    <Section title="站点">
      <TextRow
        ctx={ctx}
        group="site"
        field="app_name"
        title="站点名称"
        description="用于显示需要站点名称的地方。"
        placeholder="请输入站点名称"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="app_description"
        title="站点描述"
        description="用于显示需要站点描述的地方。"
        placeholder="请输入站点描述"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="app_url"
        title="站点网址"
        description="当前网站最新网址，将会在邮件等需要用于网址处体现。"
        placeholder="请输入站点URL，末尾不要/"
      />
      <SwitchRow
        ctx={ctx}
        group="site"
        field="force_https"
        title="强制HTTPS"
        description="当站点没有使用HTTPS，CDN或反代开启强制HTTPS时需要开启。"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="logo"
        title="LOGO"
        description="用于显示需要LOGO的地方。"
        placeholder="请输入LOGO URL，末尾不要/"
      />
      <TextareaRow
        ctx={ctx}
        group="site"
        field="subscribe_url"
        title="订阅URL"
        description="用于订阅所使用，留空则为站点URL。如需多个订阅URL随机获取请使用逗号进行分割。"
        placeholder="请输入订阅URL，末尾不要/。逗号分割支持多域名"
        rows={4}
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="subscribe_path"
        title="订阅路径"
        description="用于订阅所使用，留空则为/api/v1/client/subscribe。如需更换不同的订阅路径请设置。"
        placeholder="/api/v1/client/subscribe"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="tos_url"
        title="用户条款(TOS)URL"
        description="用于跳转到用户条款(TOS)"
        placeholder="请输入用户条款URL，末尾不要/"
      />
      <SwitchRow
        ctx={ctx}
        group="site"
        field="stop_register"
        title="停止新用户注册"
        description="开启后任何人都将无法进行注册。"
      />
      <SelectRow
        ctx={ctx}
        group="site"
        field="try_out_plan_id"
        title="注册试用"
        description="选择需要试用的订阅，如果没有选项请先前往订阅管理添加。"
        placeholder="请选择试用订阅"
        fallback="0"
        options={[
          { value: '0', label: '关闭' },
          ...plans.map((plan) => ({ value: String(plan.id), label: plan.name })),
        ]}
        serialize={selectInteger}
      />
      {tryOutOff ? null : (
        <TextRow
          ctx={ctx}
          group="site"
          field="try_out_hour"
          title="试用时间(小时)"
          placeholder="请输入"
          indent
          coerce={parseBackendNumber}
        />
      )}
      <TextRow
        ctx={ctx}
        group="site"
        field="currency"
        title="货币单位"
        description="仅用于展示使用，更改后系统中所有的货币单位都将发生变更。"
        placeholder="CNY"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="currency_symbol"
        title="货币符号"
        description="仅用于展示使用，更改后系统中所有的货币单位都将发生变更。"
        placeholder="¥"
      />
      {/* docs/api-dialect.md §10.3: boot-time legacy `#/…` → history-URL
          translation toggle, injected into both SPA runtime configs. */}
      <SwitchRow
        ctx={ctx}
        group="site"
        field="legacy_hash_redirect_enable"
        title="旧版 #/ 链接重定向"
        description="开启后旧版 /#/路径 链接将在页面加载时自动跳转到对应的新路径。"
      />
    </Section>
  );
}
