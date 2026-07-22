import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useForm, useFormState, useWatch } from 'react-hook-form';
import { useBeforeUnload, useBlocker } from 'react-router';
import type { AdminConfig, AdminConfigChanges, AdminPlanModel } from '@v2board/types';
import { getErrorPresentation, hasProblemCode } from '@v2board/api-client';
import { useSaveSystemConfigMutation } from '@/lib/queries';
import { toast } from '@v2board/app-shell/toast';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@v2board/ui/alert-dialog';
import { Button } from '@v2board/ui/button';
import {
  SECTION_FIELDS,
  SECTION_SCHEMAS,
  canonicalizeConfigDraftField,
  parseConfigServerSection,
  type ConfigFieldName,
  type ConfigFieldValue,
  type ConfigGroupKey,
  type ConfigSectionValues,
  type FormCtx,
} from './schema';
import type { PendingConfigCommit } from './pending-commit';
import { toText } from './values';
import { AppSection } from './sections/app';
import { DepositSection } from './sections/deposit';
import { EmailSection } from './sections/email';
import { FrontendSection } from './sections/frontend';
import { InviteSection } from './sections/invite';
import { SafeSection } from './sections/safe';
import { ServerSection } from './sections/server';
import { SiteSection } from './sections/site';
import { SubscribeSection } from './sections/subscribe';
import { TelegramSection } from './sections/telegram';
import { TicketSection } from './sections/ticket';

function getConfigSectionValues(
  config: AdminConfig,
  group: ConfigGroupKey,
  t: TFunction,
): ConfigSectionValues {
  const source = config[group];
  if (!source || typeof source !== 'object' || Array.isArray(source)) return {};

  const record = Object.fromEntries(Object.entries(source));
  const selected: Record<string, unknown> = {};
  for (const field of SECTION_FIELDS[group]) {
    if (Object.hasOwn(record, field)) selected[field] = record[field];
  }

  try {
    return parseConfigServerSection(group, selected);
  } catch (cause) {
    throw new Error(
      t(($) => $.admin.config.section_data_invalid, { group }),
      { cause },
    );
  }
}

type PersistResult = 'unchanged' | 'applied' | 'pending' | 'redirected' | 'failed';

const SECRET_CONFIG_FIELDS = [
  'server_token',
  'email_password',
  'telegram_bot_token',
  'recaptcha_key',
] as const satisfies readonly ConfigFieldName[];

function safeSubmittedDisplayValues(
  submitted: ConfigSectionValues,
  server: ConfigSectionValues,
): ConfigSectionValues {
  const safe = { ...submitted };
  for (const field of SECRET_CONFIG_FIELDS) {
    if (!Object.hasOwn(submitted, field)) continue;
    // Never retain a newly submitted secret as a durable UI baseline. The
    // active server projection contains only the fixed redaction sentinel (or
    // null when unset), which is safe to keep while activation is pending.
    (safe as Partial<Record<ConfigFieldName, ConfigFieldValue>>)[field] = server[field];
  }
  return safe;
}

export function ConfigDraftNavigationGuard({
  when,
  locked,
  securePathActivation = false,
  pendingRevision,
  systemNavigationAllowed,
}: {
  when: boolean;
  locked: boolean;
  securePathActivation?: boolean;
  pendingRevision?: number;
  systemNavigationAllowed: () => boolean;
}) {
  const { t } = useTranslation();
  const blocker = useBlocker(when && !systemNavigationAllowed());
  const handleBeforeUnload = useCallback(
    (event: BeforeUnloadEvent) => {
      if (!when || systemNavigationAllowed()) return;
      event.preventDefault();
      // Required by browsers that still key the native prompt off returnValue.
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      event.returnValue = '';
    },
    [systemNavigationAllowed, when],
  );
  useBeforeUnload(handleBeforeUnload, { capture: true });

  const stay = () => {
    if (blocker.state === 'blocked') blocker.reset();
  };
  const leave = () => {
    if (blocker.state === 'blocked') blocker.proceed();
  };

  return (
    <AlertDialog
      open={blocker.state === 'blocked'}
      onOpenChange={(open) => {
        if (!open) stay();
      }}
    >
      <AlertDialogContent data-testid="config-leave-dialog" className="sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle>
            {securePathActivation
              ? t(($) => $.admin.config.secure_path_activation_title)
              : locked
                ? t(($) => $.admin.config.save_success)
                : t(($) => $.admin.config.leave_prompt)}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {securePathActivation
              ? t(($) => $.admin.config.secure_path_activation_description)
              : locked
                ? t(($) => $.admin.config.activation_pending, {
                    revision: pendingRevision,
                  })
                : t(($) => $.admin.config.leave_description)}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel asChild>
            <Button type="button" variant="outline" data-testid="config-stay">
              {t(($) => $.common.cancel)}
            </Button>
          </AlertDialogCancel>
          {locked ? null : (
            <AlertDialogAction asChild>
              <Button
                type="button"
                onClick={(event) => {
                  event.preventDefault();
                  leave();
                }}
                data-testid="config-leave"
              >
                {t(($) => $.common.confirm)}
              </Button>
            </AlertDialogAction>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function SystemConfigSectionForm({
  group,
  config,
  plans,
  emailTemplates,
  onTestMail,
  testMailPending,
  onSetWebhook,
  webhookPending,
  refreshConfig,
  onDirtyChange,
  pendingCommit,
  onPendingCommit,
  onSecurePathActivated,
  systemNavigationAllowed,
}: {
  group: ConfigGroupKey;
  config: AdminConfig;
  plans?: AdminPlanModel[];
  emailTemplates?: string[];
  onTestMail: () => Promise<void>;
  testMailPending: boolean;
  onSetWebhook: (telegramBotToken: string) => Promise<void>;
  webhookPending: boolean;
  refreshConfig: () => Promise<AdminConfig>;
  onDirtyChange?: (dirty: boolean) => void;
  pendingCommit: PendingConfigCommit | null;
  onPendingCommit: (commit: PendingConfigCommit) => void;
  onSecurePathActivated: (securePath: string) => void;
  systemNavigationAllowed: () => boolean;
}) {
  const { t } = useTranslation();
  const saveConfig = useSaveSystemConfigMutation();
  const serverValues = useMemo(() => getConfigSectionValues(config, group, t), [config, group, t]);
  const form = useForm<ConfigSectionValues>({
    resolver: zodResolver(SECTION_SCHEMAS[group]),
    defaultValues: serverValues,
    mode: 'onChange',
  });
  const values = useWatch({ control: form.control });
  const { isDirty } = useFormState({ control: form.control });
  const [saving, setSaving] = useState(false);
  const [actionPending, setActionPending] = useState(false);
  const [saveError, setSaveError] = useState<string>();
  const savingRef = useRef(false);
  const actionPendingRef = useRef(false);
  // This token belongs to the server values on which the current draft was
  // based. Do not replace it when background data changes while the form is
  // dirty, or a stale draft could silently overwrite the winning edit.
  const baselineRevisionRef = useRef(config.revision);
  const pendingActivation = pendingCommit !== null;

  useEffect(() => {
    if (saving || isDirty || pendingActivation) return;
    baselineRevisionRef.current = config.revision;
    form.reset(serverValues);
  }, [config.revision, form, isDirty, pendingActivation, saving, serverValues]);

  useEffect(() => {
    onDirtyChange?.(isDirty);
    return () => onDirtyChange?.(false);
  }, [isDirty, onDirtyChange]);

  const allowedFields: readonly ConfigFieldName[] = SECTION_FIELDS[group];

  // The implementation operates at the dynamic form boundary and can return
  // the previous draft value after validation fails. FormCtx still exposes the
  // generic group/field relationship to every call site.
  const stage: FormCtx['stage'] = (
    requestedGroup: ConfigGroupKey,
    field: ConfigFieldName,
    value: unknown,
  ): ConfigFieldValue => {
    if (requestedGroup !== group || !allowedFields.includes(field)) {
      throw new Error(t(($) => $.admin.config.field_not_in_section, { field, group }));
    }
    try {
      const parsed = canonicalizeConfigDraftField(requestedGroup, field, value);
      form.clearErrors(field);
      return parsed;
    } catch (error) {
      form.setError(field, {
        type: 'validate',
        message: error instanceof Error ? error.message : t(($) => $.admin.config.invalid_value),
      });
      return form.getValues(field);
    }
  };

  const dirtyPatch = (): {
    patch: AdminConfigChanges;
    canonicalValues: ConfigSectionValues;
    valid: boolean;
  } => {
    const patch: Record<string, unknown> = {};
    const canonicalValues = { ...form.getValues() };
    let valid = true;
    for (const field of allowedFields) {
      if (!form.getFieldState(field).isDirty) continue;
      try {
        const parsed = canonicalizeConfigDraftField(group, field, form.getValues(field));
        patch[field] = parsed;
        // `field` is dynamic at this loop boundary, but the group-owned codec
        // has already proved that `parsed` is the matching field's wire type.
        // Keep the unavoidable dynamic write local instead of weakening the
        // form model or every call site to `Record<string, unknown>`.
        (canonicalValues as Partial<Record<ConfigFieldName, ConfigFieldValue>>)[field] = parsed;
        form.clearErrors(field);
      } catch (error) {
        valid = false;
        form.setError(field, {
          type: 'validate',
          message: error instanceof Error ? error.message : t(($) => $.admin.config.invalid_value),
        });
      }
    }
    return { patch: patch as AdminConfigChanges, canonicalValues, valid };
  };

  const persistDraft = async (fromAction = false): Promise<PersistResult> => {
    if (pendingActivation) return 'pending';
    if (savingRef.current || (actionPendingRef.current && !fromAction)) return 'failed';
    setSaveError(undefined);
    const valid = await form.trigger();
    if (!valid) return 'failed';
    if (savingRef.current) return 'failed';

    const { patch, canonicalValues: submittedValues, valid: patchValid } = dirtyPatch();
    if (!patchValid) return 'failed';
    if (Object.keys(patch).length === 0) return 'unchanged';
    if (
      Object.hasOwn(patch, 'secure_path') &&
      toText((patch as Record<string, unknown>).secure_path).trim() === ''
    ) {
      form.setError('secure_path', {
        type: 'validate',
        message: t(($) => $.admin.config.secure_path_required),
      });
      return 'failed';
    }

    savingRef.current = true;
    setSaving(true);
    try {
      const outcome = await saveConfig.mutateAsync({
        ...patch,
        expected_revision: baselineRevisionRef.current,
      });
      if (outcome.activation === 'pending') {
        const securePath = Object.hasOwn(patch, 'secure_path')
          ? toText((patch as Record<string, unknown>).secure_path).trim()
          : undefined;
        form.reset(safeSubmittedDisplayValues(submittedValues, serverValues));
        onPendingCommit({
          group,
          revision: outcome.revision,
          ...(securePath ? { securePath } : {}),
        });
        // A secure-path activation must be observed through its new prefix;
        // refetching the old prefix can race into a misleading 404. Ordinary
        // config commits can make one eager read before parent-level polling.
        if (!securePath) {
          try {
            await refreshConfig();
          } catch (error) {
            setSaveError(getErrorPresentation(error).message);
          }
        }
        toast.success(
          t(($) => $.admin.config.save_success),
          {
            description: t(($) => $.admin.config.save_pending_desc),
          },
        );
        return 'pending';
      }

      if (Object.hasOwn(patch, 'secure_path')) {
        form.reset(safeSubmittedDisplayValues(submittedValues, serverValues));
        toast.success(t(($) => $.admin.config.save_success));
        onSecurePathActivated(toText((patch as Record<string, unknown>).secure_path));
        return 'redirected';
      }

      form.reset(safeSubmittedDisplayValues(submittedValues, serverValues));
      try {
        const refreshed = await refreshConfig();
        baselineRevisionRef.current = refreshed.revision;
        form.reset(getConfigSectionValues(refreshed, group, t));
      } catch (error) {
        // As above, a failed read-after-write must not turn an applied PATCH
        // back into a dirty draft that a user can accidentally resubmit.
        setSaveError(getErrorPresentation(error).message);
      }
      toast.success(t(($) => $.admin.config.save_success));
      return 'applied';
    } catch (error) {
      setSaveError(getErrorPresentation(error).message);
      if (hasProblemCode(error, 'config_revision_conflict')) {
        // The losing draft remains visible; only refresh the winning snapshot.
        try {
          await refreshConfig();
        } catch {
          // The shared config query presents its own refresh failure.
        }
      }
      return 'failed';
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };

  const discardDraft = () => {
    setSaveError(undefined);
    form.clearErrors();
    baselineRevisionRef.current = config.revision;
    form.reset(serverValues);
  };

  const runTestMail = async () => {
    if (actionPendingRef.current || savingRef.current || pendingActivation) return;
    actionPendingRef.current = true;
    setActionPending(true);
    try {
      const outcome = await persistDraft(true);
      if (outcome === 'unchanged' || outcome === 'applied') await onTestMail();
    } catch {
      // Mutation failures are presented by the shared query error boundary.
    } finally {
      actionPendingRef.current = false;
      setActionPending(false);
    }
  };

  const runSetWebhook = async () => {
    if (actionPendingRef.current || savingRef.current || pendingActivation) return;
    actionPendingRef.current = true;
    setActionPending(true);
    try {
      // Capture before the authoritative refetch can replace a newly entered
      // secret with the backend's fixed redaction marker.
      const telegramBotToken = toText(form.getValues('telegram_bot_token'));
      const outcome = await persistDraft(true);
      if (outcome !== 'unchanged' && outcome !== 'applied') return;
      await onSetWebhook(telegramBotToken);
    } catch {
      // Mutation failures are presented by the shared query error boundary.
    } finally {
      actionPendingRef.current = false;
      setActionPending(false);
    }
  };

  const busy = saving || actionPending || pendingActivation;
  const ctx: FormCtx = {
    control: form.control,
    get: (requestedGroup, field) => (requestedGroup === group ? values[field] : undefined),
    isSaving: () => busy,
    stage,
  };

  let section: ReactNode = null;
  switch (group) {
    case 'site':
      if (!plans) throw new Error(t(($) => $.admin.config.site_plans_missing));
      section = <SiteSection ctx={ctx} plans={plans} />;
      break;
    case 'safe':
      section = <SafeSection ctx={ctx} />;
      break;
    case 'subscribe':
      section = <SubscribeSection ctx={ctx} />;
      break;
    case 'deposit':
      section = <DepositSection ctx={ctx} />;
      break;
    case 'ticket':
      section = <TicketSection ctx={ctx} />;
      break;
    case 'invite':
      section = <InviteSection ctx={ctx} />;
      break;
    case 'frontend':
      section = <FrontendSection ctx={ctx} />;
      break;
    case 'server':
      section = <ServerSection ctx={ctx} />;
      break;
    case 'email':
      if (!emailTemplates) throw new Error(t(($) => $.admin.config.email_templates_missing));
      section = (
        <EmailSection
          ctx={ctx}
          templates={emailTemplates}
          onTest={() => void runTestMail()}
          testing={testMailPending || actionPending || saving}
        />
      );
      break;
    case 'telegram':
      section = (
        <TelegramSection
          ctx={ctx}
          onWebhook={() => void runSetWebhook()}
          webhookPending={webhookPending || actionPending || saving}
        />
      );
      break;
    case 'app':
      section = <AppSection ctx={ctx} />;
      break;
  }

  return (
    <>
      <ConfigDraftNavigationGuard
        when={isDirty}
        locked={false}
        systemNavigationAllowed={systemNavigationAllowed}
      />
      <form
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          void persistDraft();
        }}
        noValidate
      >
        <fieldset disabled={busy} className="min-w-0 border-0 p-0">
          {section}
        </fieldset>
        {pendingCommit ? (
          <Alert data-testid="config-pending-activation">
            <AlertDescription>
              {t(($) => $.admin.config.activation_pending, {
                revision: pendingCommit.revision,
              })}
            </AlertDescription>
          </Alert>
        ) : null}
        {saveError ? (
          <Alert variant="destructive" data-testid="config-save-error">
            <AlertDescription>{saveError}</AlertDescription>
          </Alert>
        ) : null}
        <div className="flex flex-col gap-3 rounded-lg border border-border bg-card p-4 sm:flex-row sm:items-center sm:justify-between">
          <p className="text-sm text-muted-foreground" aria-live="polite">
            {isDirty
              ? t(($) => $.admin.config.unsaved_changes)
              : t(($) => $.admin.config.no_unsaved_changes)}
          </p>
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={discardDraft}
              disabled={!isDirty || busy}
              data-testid="config-discard"
            >
              {t(($) => $.admin.config.discard_changes)}
            </Button>
            <Button type="submit" disabled={!isDirty || busy} data-testid="config-save">
              {saving ? t(($) => $.admin.config.saving) : t(($) => $.admin.config.save_changes)}
            </Button>
          </div>
        </div>
      </form>
    </>
  );
}
