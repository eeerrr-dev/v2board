import { Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
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
  const hasToken = Boolean(ctx.get('telegram', 'telegram_bot_token'));
  return (
    <Section title="Telegram">
      <TextRow
        ctx={ctx}
        group="telegram"
        field="telegram_bot_token"
        title="机器人Token"
        description="请输入由Botfather提供的token。"
        placeholder="0000000000:xxxxxxxxx_xxxxxxxxxxxxxxx"
      />
      {hasToken ? (
        <SettingRow
          title="设置Webhook"
          description="对机器人进行Webhook设置，不设置将无法收到Telegram通知。"
        >
          <Button onClick={onWebhook} disabled={webhookPending} data-testid="config-set-webhook">
            {webhookPending ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            一键设置
          </Button>
        </SettingRow>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="telegram"
        field="telegram_bot_enable"
        title="开启机器人通知"
        description="开启后bot将会对绑定了telegram的管理员和用户进行基础通知。"
      />
      <TextRow
        ctx={ctx}
        group="telegram"
        field="telegram_discuss_link"
        title="群组地址"
        description="填写后将会在用户端展示，或者被用于需要的地方。"
        placeholder="https://t.me/xxxxxx"
      />
    </Section>
  );
}
