import { useTranslation } from 'react-i18next';
import type { Plan } from '@v2board/types';
import type { FormCtx } from '../schema';
import { Section, SelectRow, SwitchRow, TextRow, TextareaRow } from '../rows';
import { parseBackendNumber, selectInteger } from '../values';

export function SiteSection({ ctx, plans }: { ctx: FormCtx; plans: Plan[] }) {
  const { t } = useTranslation();
  const tryOutOff = String(ctx.get('site', 'try_out_plan_id') ?? 0) === '0';
  return (
    <Section title={t(($) => $.admin.config.sections.site)}>
      <TextRow
        ctx={ctx}
        group="site"
        field="app_name"
        title={t(($) => $.admin.config.site.app_name_title)}
        description={t(($) => $.admin.config.site.app_name_desc)}
        placeholder={t(($) => $.admin.config.site.app_name_placeholder)}
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="app_description"
        title={t(($) => $.admin.config.site.app_description_title)}
        description={t(($) => $.admin.config.site.app_description_desc)}
        placeholder={t(($) => $.admin.config.site.app_description_placeholder)}
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="app_url"
        title={t(($) => $.admin.config.site.app_url_title)}
        description={t(($) => $.admin.config.site.app_url_desc)}
        placeholder={t(($) => $.admin.config.site.app_url_placeholder)}
      />
      <SwitchRow
        ctx={ctx}
        group="site"
        field="force_https"
        title={t(($) => $.admin.config.site.force_https_title)}
        description={t(($) => $.admin.config.site.force_https_desc)}
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="logo"
        title="LOGO"
        description={t(($) => $.admin.config.site.logo_desc)}
        placeholder={t(($) => $.admin.config.site.logo_placeholder)}
      />
      <TextareaRow
        ctx={ctx}
        group="site"
        field="subscribe_url"
        title={t(($) => $.admin.config.site.subscribe_url_title)}
        description={t(($) => $.admin.config.site.subscribe_url_desc)}
        placeholder={t(($) => $.admin.config.site.subscribe_url_placeholder)}
        rows={4}
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="subscribe_path"
        title={t(($) => $.admin.config.site.subscribe_path_title)}
        description={t(($) => $.admin.config.site.subscribe_path_desc)}
        placeholder="/api/v1/client/subscribe"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="tos_url"
        title={t(($) => $.admin.config.site.tos_url_title)}
        description={t(($) => $.admin.config.site.tos_url_desc)}
        placeholder={t(($) => $.admin.config.site.tos_url_placeholder)}
      />
      <SwitchRow
        ctx={ctx}
        group="site"
        field="stop_register"
        title={t(($) => $.admin.config.site.stop_register_title)}
        description={t(($) => $.admin.config.site.stop_register_desc)}
      />
      <SelectRow
        ctx={ctx}
        group="site"
        field="try_out_plan_id"
        title={t(($) => $.admin.config.site.try_out_plan_title)}
        description={t(($) => $.admin.config.site.try_out_plan_desc)}
        placeholder={t(($) => $.admin.config.site.try_out_plan_placeholder)}
        fallback="0"
        options={[
          { value: '0', label: t(($) => $.common.close) },
          ...plans.map((plan) => ({ value: String(plan.id), label: plan.name })),
        ]}
        serialize={selectInteger}
      />
      {tryOutOff ? null : (
        <TextRow
          ctx={ctx}
          group="site"
          field="try_out_hour"
          title={t(($) => $.admin.config.site.try_out_hour_title)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
          indent
          coerce={parseBackendNumber}
        />
      )}
      <TextRow
        ctx={ctx}
        group="site"
        field="currency"
        title={t(($) => $.admin.config.site.currency_title)}
        description={t(($) => $.admin.config.site.currency_desc)}
        placeholder="CNY"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="currency_symbol"
        title={t(($) => $.admin.config.site.currency_symbol_title)}
        description={t(($) => $.admin.config.site.currency_desc)}
        placeholder="¥"
      />
      {/* docs/api-dialect.md §10.3: boot-time legacy `#/…` → history-URL
          translation toggle, injected into both SPA runtime configs. */}
      <SwitchRow
        ctx={ctx}
        group="site"
        field="legacy_hash_redirect_enable"
        title={t(($) => $.admin.config.site.legacy_hash_redirect_title)}
        description={t(($) => $.admin.config.site.legacy_hash_redirect_desc)}
      />
    </Section>
  );
}
