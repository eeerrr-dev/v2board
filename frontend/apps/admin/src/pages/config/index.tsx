import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import { admin } from '@v2board/api-client';
import {
  useAdminPlans,
  useConfig,
  useEmailTemplates,
  useSetTelegramWebhookMutation,
  useTestSendMailMutation,
} from '@/lib/queries';
import { apiClient } from '@/lib/api';
import { getAdminSecurePath } from '@/lib/runtime-config';
import { cn } from '@v2board/ui/cn';
import { toast } from '@v2board/app-shell/toast';
import { PageHeader, PageShell } from '@v2board/ui/page';
import { ErrorState } from '@v2board/ui/error-state';
import { LoadingState, SkeletonFields } from '@v2board/ui/loading-state';
import { SECTIONS, type ConfigGroupKey } from './schema';
import { ConfigDraftNavigationGuard, SystemConfigSectionForm } from './section-form';
import {
  clearPendingConfigCommit,
  readPendingConfigCommit,
  subscribePendingConfigCommit,
  writePendingConfigCommit,
  type PendingConfigCommit,
} from './pending-commit';
import { replaceAdminSecurePath } from './values';

export default function ConfigPage() {
  return <SystemConfigPage />;
}

// ---------------------------------------------------------------------------
// System config (one explicit draft transaction per setting group)
// ---------------------------------------------------------------------------

function SystemConfigPage() {
  const { t } = useTranslation();
  const pendingCommit = useSyncExternalStore(
    subscribePendingConfigCommit,
    readPendingConfigCommit,
    () => null,
  );
  const config = useConfig(undefined, pendingCommit && !pendingCommit.securePath ? 2_000 : false);
  const plans = useAdminPlans();
  const emailTemplates = useEmailTemplates();
  const webhook = useSetTelegramWebhookMutation();
  const testMail = useTestSendMailMutation();
  const [active, setActive] = useState<ConfigGroupKey>('site');
  const [draftDirty, setDraftDirty] = useState(false);
  const systemNavigationAllowedRef = useRef(false);
  const systemNavigationAllowed = useCallback(() => systemNavigationAllowedRef.current, []);

  const activateSecurePath = useCallback((securePath: string) => {
    systemNavigationAllowedRef.current = true;
    replaceAdminSecurePath(securePath);
  }, []);

  const finishPendingCommit = useCallback(
    (commit: PendingConfigCommit) => {
      if (!clearPendingConfigCommit(commit)) return;
      if (commit.securePath && getAdminSecurePath() !== commit.securePath) {
        activateSecurePath(commit.securePath);
      }
    },
    [activateSecurePath],
  );

  const recordPendingCommit = useCallback((commit: PendingConfigCommit) => {
    writePendingConfigCommit(commit);
  }, []);

  // For an ordinary config write, the existing prefix remains valid and the
  // query polls until its active revision reaches the durable commit.
  useEffect(() => {
    if (
      pendingCommit &&
      !pendingCommit.securePath &&
      config.data &&
      config.data.revision >= pendingCommit.revision
    ) {
      finishPendingCommit(pendingCommit);
    }
  }, [config.data, finishPendingCommit, pendingCommit]);

  // A secure-path activation can invalidate the old endpoint before it can
  // report the new revision. Probe the newly committed prefix directly and
  // redirect only after that prefix serves the target revision.
  useEffect(() => {
    const commit = pendingCommit;
    if (!commit) return;
    const securePath = commit.securePath;
    if (!securePath) return;
    const controller = new AbortController();
    let stopped = false;
    let timer: number | undefined;

    const probe = async () => {
      try {
        const active = await admin.fetchConfigAtAdminPath(apiClient, securePath, 'safe', {
          signal: controller.signal,
        });
        if (!stopped && active.revision >= commit.revision) {
          finishPendingCommit(commit);
          return;
        }
      } catch {
        // Before activation the new dynamic prefix is expected to be absent.
        // The next bounded poll retries; no PATCH is ever repeated.
      }
      if (!stopped) timer = window.setTimeout(() => void probe(), 2_000);
    };

    void probe();
    return () => {
      stopped = true;
      controller.abort();
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [finishPendingCommit, pendingCommit]);

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

  const pendingNavigationGuard = pendingCommit ? (
    <ConfigDraftNavigationGuard
      when
      locked
      securePathActivation={pendingCommit.securePath !== undefined}
      pendingRevision={pendingCommit.revision}
      systemNavigationAllowed={systemNavigationAllowed}
    />
  ) : null;

  if (config.isError) {
    if (pendingCommit?.securePath) {
      return (
        <>
          <PageShell data-testid="config-page">
            <LoadingState
              className="py-6"
              label={t(($) => $.admin.config.activation_pending, {
                revision: pendingCommit.revision,
              })}
            />
          </PageShell>
          {pendingNavigationGuard}
        </>
      );
    }
    return (
      <>
        <PageShell data-testid="config-page">
          <ErrorState
            message={t(($) => $.admin.config.load_failed)}
            onRetry={() => void config.refetch()}
          />
        </PageShell>
        {pendingNavigationGuard}
      </>
    );
  }

  if (config.isPending || !config.data) {
    return (
      <>
        <PageShell data-testid="config-page">
          <LoadingState className="py-6">
            <SkeletonFields fields={5} />
          </LoadingState>
        </PageShell>
        {pendingNavigationGuard}
      </>
    );
  }

  return (
    <>
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
                disabled={draftDirty && active !== section.key}
                aria-current={active === section.key ? 'page' : undefined}
                data-testid={`config-tab-${section.key}`}
                className={cn(
                  'rounded-md px-3 py-2 text-left text-sm font-medium transition-colors',
                  active === section.key
                    ? 'bg-muted text-foreground'
                    : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground',
                  'disabled:cursor-not-allowed disabled:opacity-50',
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
                onDirtyChange={setDraftDirty}
                pendingCommit={pendingCommit}
                onPendingCommit={recordPendingCommit}
                onSecurePathActivated={activateSecurePath}
                systemNavigationAllowed={systemNavigationAllowed}
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
      {pendingNavigationGuard}
    </>
  );
}

function ConfigDependencyLoading({ label }: { label: string }) {
  return (
    <LoadingState className="py-6" label={label}>
      <SkeletonFields fields={3} />
    </LoadingState>
  );
}
