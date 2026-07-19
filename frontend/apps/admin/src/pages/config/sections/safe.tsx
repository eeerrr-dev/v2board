import { useTranslation } from 'react-i18next';
import type { FormCtx } from '../schema';
import { Section, SwitchRow, TextRow, TextareaRow } from '../rows';
import { isBackendEnabled, parseBackendInteger, splitComma } from '../values';

export function SafeSection({ ctx }: { ctx: FormCtx }) {
  const { t } = useTranslation();
  return (
    <Section title={t(($) => $.admin.config.sections.safe)}>
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_verify"
        title={t(($) => $.admin.config.safe.email_verify_title)}
        description={t(($) => $.admin.config.safe.email_verify_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_gmail_limit_enable"
        title={t(($) => $.admin.config.safe.gmail_limit_title)}
        description={t(($) => $.admin.config.safe.gmail_limit_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="safe_mode_enable"
        title={t(($) => $.admin.config.safe.safe_mode_title)}
        description={t(($) => $.admin.config.safe.safe_mode_desc)}
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="admin_mfa_force"
        title={t(($) => $.admin.config.safe.admin_mfa_force_title)}
        description={t(($) => $.admin.config.safe.admin_mfa_force_desc)}
      />
      <TextRow
        ctx={ctx}
        group="safe"
        field="secure_path"
        title={t(($) => $.admin.config.safe.secure_path_title)}
        description={t(($) => $.admin.config.safe.secure_path_desc)}
        placeholder="admin"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_whitelist_enable"
        title={t(($) => $.admin.config.safe.email_whitelist_title)}
        description={t(($) => $.admin.config.safe.email_whitelist_desc)}
      />
      {isBackendEnabled(ctx.get('safe', 'email_whitelist_enable')) ? (
        <TextareaRow
          ctx={ctx}
          group="safe"
          field="email_whitelist_suffix"
          title={t(($) => $.admin.config.safe.whitelist_suffix_title)}
          description={t(($) => $.admin.config.safe.whitelist_suffix_desc)}
          placeholder={t(($) => $.admin.config.safe.whitelist_suffix_placeholder)}
          rows={4}
          indent
          coerce={splitComma}
        />
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="recaptcha_enable"
        title={t(($) => $.admin.config.safe.recaptcha_title)}
        description={t(($) => $.admin.config.safe.recaptcha_desc)}
      />
      {isBackendEnabled(ctx.get('safe', 'recaptcha_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="recaptcha_key"
            title={t(($) => $.admin.config.safe.recaptcha_key_title)}
            description={t(($) => $.admin.config.safe.recaptcha_key_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="recaptcha_site_key"
            title={t(($) => $.admin.config.safe.recaptcha_site_key_title)}
            description={t(($) => $.admin.config.safe.recaptcha_site_key_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
          />
        </>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="register_limit_by_ip_enable"
        title={t(($) => $.admin.config.safe.register_limit_title)}
        description={t(($) => $.admin.config.safe.register_limit_desc)}
      />
      {isBackendEnabled(ctx.get('safe', 'register_limit_by_ip_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="register_limit_count"
            title={t(($) => $.admin.config.safe.count_title)}
            description={t(($) => $.admin.config.safe.register_limit_count_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
            coerce={parseBackendInteger}
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="register_limit_expire"
            title={t(($) => $.admin.config.safe.penalty_minutes_title)}
            description={t(($) => $.admin.config.safe.register_limit_expire_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
            coerce={parseBackendInteger}
          />
        </>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="password_limit_enable"
        title={t(($) => $.admin.config.safe.password_limit_title)}
        description={t(($) => $.admin.config.safe.password_limit_desc)}
      />
      {isBackendEnabled(ctx.get('safe', 'password_limit_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="password_limit_count"
            title={t(($) => $.admin.config.safe.count_title)}
            description={t(($) => $.admin.config.safe.password_limit_count_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
            coerce={parseBackendInteger}
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="password_limit_expire"
            title={t(($) => $.admin.config.safe.penalty_minutes_title)}
            description={t(($) => $.admin.config.safe.password_limit_expire_desc)}
            placeholder={t(($) => $.admin.config.input_placeholder)}
            indent
            coerce={parseBackendInteger}
          />
        </>
      ) : null}
    </Section>
  );
}
