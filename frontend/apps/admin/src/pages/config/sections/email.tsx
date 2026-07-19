import { Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import type { FormCtx } from '../schema';
import { Section, SelectRow, SettingRow, TextRow, WarningAlert } from '../rows';
import { parseBackendInteger } from '../values';

export function EmailSection({
  ctx,
  templates,
  onTest,
  testing,
}: {
  ctx: FormCtx;
  templates: string[];
  onTest: () => void;
  testing: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <WarningAlert>{t(($) => $.admin.config.email.save_notice)}</WarningAlert>
      <Section title={t(($) => $.admin.config.sections.email)}>
        <TextRow
          ctx={ctx}
          group="email"
          field="email_host"
          title={t(($) => $.admin.config.email.host_title)}
          description={t(($) => $.admin.config.email.host_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_port"
          title={t(($) => $.admin.config.email.port_title)}
          description={t(($) => $.admin.config.email.port_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
          coerce={parseBackendInteger}
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_encryption"
          title={t(($) => $.admin.config.email.encryption_title)}
          description={t(($) => $.admin.config.email.encryption_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_username"
          title={t(($) => $.admin.config.email.username_title)}
          description={t(($) => $.admin.config.email.username_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_password"
          title={t(($) => $.admin.config.email.password_title)}
          description={t(($) => $.admin.config.email.password_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_from_address"
          title={t(($) => $.admin.config.email.from_address_title)}
          description={t(($) => $.admin.config.email.from_address_desc)}
          placeholder={t(($) => $.admin.config.input_placeholder)}
        />
        <SelectRow
          ctx={ctx}
          group="email"
          field="email_template"
          title={t(($) => $.admin.config.email.template_title)}
          description={t(($) => $.admin.config.email.template_desc)}
          options={templates.map((template) => ({ value: template, label: template }))}
        />
        <SettingRow
          title={t(($) => $.admin.config.email.send_test_mail)}
          description={t(($) => $.admin.config.email.send_test_mail_desc)}
        >
          <Button onClick={onTest} disabled={testing} data-testid="config-test-mail">
            {testing ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            {t(($) => $.admin.config.email.send_test_mail)}
          </Button>
        </SettingRow>
      </Section>
    </div>
  );
}
