import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SwitchRow, TextRow, TextareaRow } from '../rows';
import { isBackendEnabled } from '../values';

export function InviteSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  return (
    <Section title={t(($) => $.admin.config.sections.invite)}>
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="invite_force"
        title={t(($) => $.admin.config.invite.force_title)}
        description={t(($) => $.admin.config.invite.force_desc)}
      />
      <TextRow
        ctx={ctx}
        group="invite"
        field="invite_commission"
        title={t(($) => $.admin.config.invite.commission_title)}
        description={t(($) => $.admin.config.invite.commission_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
      />
      <TextRow
        ctx={ctx}
        group="invite"
        field="invite_gen_limit"
        title={t(($) => $.admin.config.invite.gen_limit_title)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="invite_never_expire"
        title={t(($) => $.admin.config.invite.never_expire_title)}
        description={t(($) => $.admin.config.invite.never_expire_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="commission_first_time_enable"
        title={t(($) => $.admin.config.invite.first_time_title)}
        description={t(($) => $.admin.config.invite.first_time_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="commission_auto_check_enable"
        title={t(($) => $.admin.config.invite.auto_check_title)}
        description={t(($) => $.admin.config.invite.auto_check_desc)}
      />
      <TextRow
        ctx={ctx}
        group="invite"
        field="commission_withdraw_limit"
        title={t(($) => $.admin.config.invite.withdraw_limit_title)}
        description={t(($) => $.admin.config.invite.withdraw_limit_desc)}
        placeholder={t(($) => $.admin.config.input_placeholder)}
      />
      <TextareaRow
        ctx={ctx}
        group="invite"
        field="commission_withdraw_method"
        title={t(($) => $.admin.config.invite.withdraw_method_title)}
        description={t(($) => $.admin.config.invite.withdraw_method_desc)}
        placeholder={t(($) => $.admin.config.invite.withdraw_method_placeholder)}
        rows={4}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="withdraw_close_enable"
        title={t(($) => $.admin.config.invite.withdraw_close_title)}
        description={t(($) => $.admin.config.invite.withdraw_close_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="commission_distribution_enable"
        title={t(($) => $.admin.config.invite.distribution_title)}
        description={t(($) => $.admin.config.invite.distribution_desc)}
      />
      {isBackendEnabled(ctx.get('invite', 'commission_distribution_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="invite"
            field="commission_distribution_l1"
            title={t(($) => $.admin.config.invite.distribution_l1_title)}
            placeholder={t(($) => $.admin.config.invite.distribution_l1_placeholder)}
            indent
          />
          <TextRow
            ctx={ctx}
            group="invite"
            field="commission_distribution_l2"
            title={t(($) => $.admin.config.invite.distribution_l2_title)}
            placeholder={t(($) => $.admin.config.invite.distribution_l2_placeholder)}
            indent
          />
          <TextRow
            ctx={ctx}
            group="invite"
            field="commission_distribution_l3"
            title={t(($) => $.admin.config.invite.distribution_l3_title)}
            placeholder={t(($) => $.admin.config.invite.distribution_l3_placeholder)}
            indent
          />
        </>
      ) : null}
    </Section>
  );
}
