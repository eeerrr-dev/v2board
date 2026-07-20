import { Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@v2board/ui/button';
import type { FormCtx } from '../schema';
import { Section, SettingRow, SwitchRow, TextRow } from '../rows';

export function TelegramSection({
  ctx,
  onWebhook,
  webhookPending,
}: {
  ctx: FormCtx;
  onWebhook: () => void;
  webhookPending: boolean;
}) {
  const { t } = useTranslation();
  const hasToken = Boolean(ctx.get('telegram', 'telegram_bot_token'));
  return (
    <Section title={t(($) => $.admin.config.sections.telegram)}>
      <TextRow
        ctx={ctx}
        group="telegram"
        field="telegram_bot_token"
        title={t(($) => $.admin.config.telegram.bot_token_title)}
        description={t(($) => $.admin.config.telegram.bot_token_desc)}
        placeholder="0000000000:xxxxxxxxx_xxxxxxxxxxxxxxx"
      />
      {hasToken ? (
        <SettingRow
          title={t(($) => $.admin.config.telegram.webhook_title)}
          description={t(($) => $.admin.config.telegram.webhook_desc)}
        >
          <Button
            type="button"
            onClick={onWebhook}
            disabled={webhookPending}
            data-testid="config-set-webhook"
          >
            {webhookPending ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            {t(($) => $.admin.config.telegram.webhook_button)}
          </Button>
        </SettingRow>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="telegram"
        field="telegram_bot_enable"
        title={t(($) => $.admin.config.telegram.bot_enable_title)}
        description={t(($) => $.admin.config.telegram.bot_enable_desc)}
      />
      <TextRow
        ctx={ctx}
        group="telegram"
        field="telegram_discuss_link"
        title={t(($) => $.admin.config.telegram.discuss_link_title)}
        description={t(($) => $.admin.config.telegram.discuss_link_desc)}
        placeholder="https://t.me/xxxxxx"
      />
    </Section>
  );
}
