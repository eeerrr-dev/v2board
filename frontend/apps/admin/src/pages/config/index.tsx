import { useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
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
import { LoadingState, SkeletonFields } from '@/components/ui/loading-state';
import { SECTIONS, type ConfigGroupKey } from './schema';
import { SystemConfigSectionForm } from './section-form';

export default function ConfigPage() {
  return <SystemConfigPage />;
}

// ---------------------------------------------------------------------------
// System config (grouped setting fields, auto-saved per field to /config/save)
// ---------------------------------------------------------------------------

function SystemConfigPage() {
  const { t } = useTranslation();
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
      toast.success(
        t(($) => $.admin.config.send_success),
        result.log ? { description: result.log } : undefined,
      );
    } else {
      toast.error(
        t(($) => $.admin.config.send_fail),
        result.log ? { description: result.log } : undefined,
      );
    }
  };

  const setWebhook = async (telegramBotToken: string) => {
    await webhook.mutateAsync(telegramBotToken);
    toast.success(t(($) => $.admin.config.webhook_success));
  };

  if (config.isError) {
    return (
      <PageShell data-testid="config-page">
        <ErrorState
          message={t(($) => $.admin.config.load_failed)}
          onRetry={() => void config.refetch()}
        />
      </PageShell>
    );
  }

  if (config.isPending || !config.data) {
    return (
      <PageShell data-testid="config-page">
        <LoadingState className="py-6">
          <SkeletonFields fields={5} />
        </LoadingState>
      </PageShell>
    );
  }

  return (
    <PageShell data-testid="config-page">
      <PageHeader
        title={t(($) => $.admin.config.title)}
        description={t(($) => $.admin.config.description)}
      />

      <div className="grid gap-6 @3xl/main:grid-cols-[180px_1fr] @3xl/main:items-start">
        <nav
          className="flex flex-row flex-wrap gap-1 @3xl/main:sticky @3xl/main:top-4 @3xl/main:flex-col"
          aria-label={t(($) => $.admin.config.nav_label)}
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
              {section.title(t)}
            </button>
          ))}
        </nav>

        <div className="min-w-0 space-y-6">
          {active === 'site' && plans.isError ? (
            <ErrorState
              message={t(($) => $.admin.config.plans_dep_error)}
              onRetry={() => void plans.refetch()}
              data-testid="config-plans-error"
            />
          ) : active === 'site' && !plans.data ? (
            <ConfigDependencyLoading label={t(($) => $.admin.config.plans_dep_loading)} />
          ) : active === 'email' && emailTemplates.isError ? (
            <ErrorState
              message={t(($) => $.admin.config.email_templates_error)}
              onRetry={() => void emailTemplates.refetch()}
              data-testid="config-email-templates-error"
            />
          ) : active === 'email' && !emailTemplates.data ? (
            <ConfigDependencyLoading label={t(($) => $.admin.config.email_templates_loading)} />
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
                  throw result.error ?? new Error(t(($) => $.admin.config.refresh_failed));
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
    <LoadingState className="py-6" label={label}>
      <SkeletonFields fields={3} />
    </LoadingState>
  );
}
