import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SelectRow, SwitchRow, TextRow, orderEventOptions } from '../rows';
import { selectBoolean, selectInteger } from '../values';

export function SubscribeSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  const timedExpire = String(ctx.get('subscribe', 'show_subscribe_method') ?? 0) === '2';
  const eventOptions = orderEventOptions(t);
  return (
    <Section title={t(($) => $.admin.config.sections.subscribe)}>
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="plan_change_enable"
        title={t(($) => $.admin.config.subscribe.plan_change_title)}
        description={t(($) => $.admin.config.subscribe.plan_change_desc)}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="reset_traffic_method"
        title={t(($) => $.admin.config.subscribe.reset_method_title)}
        description={t(($) => $.admin.config.subscribe.reset_method_desc)}
        placeholder={t(($) => $.admin.config.subscribe.reset_method_placeholder)}
        fallback="0"
        options={[
          { value: '0', label: t(($) => $.admin.config.subscribe.reset_method_first_day) },
          { value: '1', label: t(($) => $.admin.config.subscribe.reset_method_monthly) },
          { value: '2', label: t(($) => $.admin.config.subscribe.reset_method_none) },
          { value: '3', label: t(($) => $.admin.config.subscribe.reset_method_year_first_day) },
          { value: '4', label: t(($) => $.admin.config.subscribe.reset_method_yearly) },
        ]}
        serialize={selectInteger}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="surplus_enable"
        title={t(($) => $.admin.config.subscribe.surplus_title)}
        description={t(($) => $.admin.config.subscribe.surplus_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="allow_new_period"
        title={t(($) => $.admin.config.subscribe.new_period_title)}
        description={t(($) => $.admin.config.subscribe.new_period_desc)}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="new_order_event_id"
        title={t(($) => $.admin.config.subscribe.new_order_event_title)}
        description={t(($) => $.admin.config.subscribe.new_order_event_desc)}
        placeholder={t(($) => $.admin.config.subscribe.event_placeholder)}
        fallback="0"
        options={eventOptions}
        serialize={selectBoolean}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="renew_order_event_id"
        title={t(($) => $.admin.config.subscribe.renew_order_event_title)}
        description={t(($) => $.admin.config.subscribe.renew_order_event_desc)}
        placeholder={t(($) => $.admin.config.subscribe.event_placeholder)}
        fallback="0"
        options={eventOptions}
        serialize={selectBoolean}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="change_order_event_id"
        title={t(($) => $.admin.config.subscribe.change_order_event_title)}
        description={t(($) => $.admin.config.subscribe.change_order_event_desc)}
        placeholder={t(($) => $.admin.config.subscribe.event_placeholder)}
        fallback="0"
        options={eventOptions}
        serialize={selectBoolean}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="show_info_to_server_enable"
        title={t(($) => $.admin.config.subscribe.show_info_title)}
        description={t(($) => $.admin.config.subscribe.show_info_desc)}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="show_subscribe_method"
        title={t(($) => $.admin.config.subscribe.show_method_title)}
        description={t(($) => $.admin.config.subscribe.show_method_desc)}
        placeholder={t(($) => $.admin.config.select_placeholder)}
        fallback="0"
        options={[
          { value: '0', label: t(($) => $.admin.config.subscribe.show_method_permanent) },
          { value: '1', label: t(($) => $.admin.config.subscribe.show_method_once) },
          { value: '2', label: t(($) => $.admin.config.subscribe.show_method_timed) },
        ]}
        serialize={selectInteger}
      />
      {timedExpire ? (
        <TextRow
          ctx={ctx}
          group="subscribe"
          field="show_subscribe_expire"
          title={t(($) => $.admin.config.subscribe.show_expire_title)}
          description={t(($) => $.admin.config.subscribe.show_expire_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
          indent
        />
      ) : null}
    </Section>
  );
}
