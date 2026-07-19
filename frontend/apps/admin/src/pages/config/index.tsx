import { useCallback, useRef, useState } from 'react';
import {
  useAdminPlans,
  useConfig,
  useEmailTemplates,
  useSetTelegramWebhookMutation,
  useTestSendMailMutation,
} from '@/lib/queries';
import { cn } from '@/lib/cn';
import { toast } from '@/lib/toast';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { Spinner } from '@/components/ui/spinner';
import { SECTIONS, type ConfigGroupKey } from './schema';
import { SystemConfigSectionForm } from './section-form';

export default function ConfigPage() {
  return <SystemConfigPage />;
}

// ---------------------------------------------------------------------------
// System config (grouped setting fields, auto-saved per field to /config/save)
// ---------------------------------------------------------------------------

function SystemConfigPage() {
  const config = useConfig();
  const plans = useAdminPlans();
  const emailTemplates = useEmailTemplates();
  const webhook = useSetTelegramWebhookMutation();
  const testMail = useTestSendMailMutation();
  const [active, setActive] = useState<ConfigGroupKey>('site');
  const saveTail = useRef<Promise<void>>(Promise.resolve());
  const serializeConfigSave = useCallback(<T,>(operation: () => Promise<T>): Promise<T> => {
    const result = saveTail.current.then(operation);
    saveTail.current = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }, []);

  const sendTestMail = async () => {
    // §6.1: bare `{sent, log}` — the native probe is synchronous, so failures
    // arrive as problems (handled by the mutation error path) and `log` is a
    // nullable transcript line, not a legacy log object.
    const result = await testMail.mutateAsync();
    if (result.sent) {
      toast.success('发送成功', result.log ? { description: result.log } : undefined);
    } else {
      toast.error('发送失败', result.log ? { description: result.log } : undefined);
    }
  };

  const setWebhook = async (telegramBotToken: string) => {
    await webhook.mutateAsync(telegramBotToken);
    toast.success('webhook 设置成功');
  };

  if (config.isError) {
    return (
      <PageShell data-testid="config-page">
        <ErrorState message="系统配置加载失败" onRetry={() => void config.refetch()} />
      </PageShell>
    );
  }

  if (config.isPending || !config.data) {
    return (
      <PageShell data-testid="config-page">
        <div className="flex justify-center py-16" role="status">
          <Spinner className="size-6 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      </PageShell>
    );
  }

  return (
    <PageShell data-testid="config-page">
      <PageHeader title="系统配置" description="所有配置修改后会自动保存并对全站生效。" />

      <div className="grid gap-6 @3xl/main:grid-cols-[180px_1fr] @3xl/main:items-start">
        <nav
          className="flex flex-row flex-wrap gap-1 @3xl/main:sticky @3xl/main:top-4 @3xl/main:flex-col"
          aria-label="配置分组"
        >
          {SECTIONS.map((section) => (
            <button
              key={section.key}
              type="button"
              onClick={() => setActive(section.key)}
              aria-current={active === section.key ? 'page' : undefined}
              data-testid={`config-tab-${section.key}`}
              className={cn(
                'rounded-md px-3 py-2 text-left text-sm font-medium transition-colors',
                active === section.key
                  ? 'bg-muted text-foreground'
                  : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground',
              )}
            >
              {section.title}
            </button>
          ))}
        </nav>

        <div className="min-w-0 space-y-6">
          {active === 'site' && plans.isError ? (
            <ErrorState
              message="订阅依赖加载失败，无法编辑站点配置"
              onRetry={() => void plans.refetch()}
              data-testid="config-plans-error"
            />
          ) : active === 'site' && !plans.data ? (
            <ConfigDependencyLoading label="正在加载订阅依赖" />
          ) : active === 'email' && emailTemplates.isError ? (
            <ErrorState
              message="邮件模板加载失败，无法编辑邮件配置"
              onRetry={() => void emailTemplates.refetch()}
              data-testid="config-email-templates-error"
            />
          ) : active === 'email' && !emailTemplates.data ? (
            <ConfigDependencyLoading label="正在加载邮件模板" />
          ) : (
            <SystemConfigSectionForm
              key={active}
              group={active}
              config={config.data}
              plans={plans.data}
              emailTemplates={emailTemplates.data}
              onTestMail={sendTestMail}
              testMailPending={testMail.isPending}
              onSetWebhook={setWebhook}
              webhookPending={webhook.isPending}
              serializeConfigSave={serializeConfigSave}
              refreshConfig={async () => {
                const result = await config.refetch();
                if (result.isError || !result.data) {
                  throw result.error ?? new Error('系统配置刷新失败');
                }
                return result.data;
              }}
            />
          )}
        </div>
      </div>
    </PageShell>
  );
}

function ConfigDependencyLoading({ label }: { label: string }) {
  return (
    <div className="flex justify-center py-16" role="status">
      <Spinner className="size-6 text-muted-foreground" />
      <span className="sr-only">{label}</span>
    </div>
  );
}
