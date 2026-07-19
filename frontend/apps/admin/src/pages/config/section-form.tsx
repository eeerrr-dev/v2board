import { useEffect, useMemo, useRef, useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useForm, useWatch } from 'react-hook-form';
import type { AdminConfig, AdminConfigFlat, Plan } from '@v2board/types';
import { getErrorPresentation, hasProblemCode } from '@v2board/api-client';
import { useSaveSystemConfigMutation } from '@/lib/queries';
import { toast } from '@/lib/toast';
import {
  SECTION_FIELDS,
  SECTION_SCHEMAS,
  configFieldValueSchema,
  type ConfigFieldValue,
  type ConfigGroupKey,
  type ConfigSectionValues,
  type FormCtx,
  type SerializedConfigSave,
} from './schema';
import { configValuesEqual, replaceAdminSecurePath, toText } from './values';
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

  const parsed = SECTION_SCHEMAS[group].safeParse(selected);
  if (!parsed.success) {
    throw new Error(
      t(($) => $.admin.config.section_data_invalid, { group }),
      {
        cause: parsed.error,
      },
    );
  }
  return parsed.data;
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
  serializeConfigSave,
  refreshConfig,
}: {
  group: ConfigGroupKey;
  config: AdminConfig;
  plans?: Plan[];
  emailTemplates?: string[];
  onTestMail: () => Promise<void>;
  testMailPending: boolean;
  onSetWebhook: (telegramBotToken: string) => Promise<void>;
  webhookPending: boolean;
  serializeConfigSave: SerializedConfigSave;
  refreshConfig: () => Promise<AdminConfig>;
}) {
  const { t } = useTranslation();
  const saveConfig = useSaveSystemConfigMutation();
  // Deliberate useMemo: serverValues identity feeds the draft-reset effect
  // below; recomputing per render would clobber in-progress field edits.
  const serverValues = useMemo(() => getConfigSectionValues(config, group, t), [config, group, t]);
  const form = useForm<ConfigSectionValues>({
    resolver: zodResolver(SECTION_SCHEMAS[group]),
    defaultValues: serverValues,
    mode: 'onChange',
  });
  const values = useWatch({ control: form.control });
  const [pendingFields, setPendingFields] = useState<ReadonlySet<string>>(() => new Set());
  const [actionPending, setActionPending] = useState(false);
  const queuedSaves = useRef(
    new Map<
      string,
      { generation: number; value: ConfigFieldValue; draftValue: ConfigFieldValue }
    >(),
  );
  const drainingFields = useRef(new Set<string>());
  const drainPromises = useRef(new Map<string, Promise<void>>());
  const saveFailures = useRef(new Map<string, { generation: number; error: unknown }>());
  const saveGenerations = useRef<Record<string, number>>({});
  const canonicalValues = useRef<ConfigSectionValues>({ ...serverValues });
  const refreshTail = useRef<Promise<void>>(Promise.resolve());
  const mounted = useRef(true);

  useEffect(() => {
    // A full-config refetch updates the shared query before its promise settles.
    // Preserve every field whose save queue is still draining so an older
    // response cannot briefly or permanently replace the latest local value.
    const currentValues = form.getValues();
    const nextValues = { ...serverValues };
    for (const field of drainingFields.current) {
      if (Object.hasOwn(currentValues, field)) nextValues[field] = currentValues[field];
    }
    for (const field of queuedSaves.current.keys()) {
      if (Object.hasOwn(currentValues, field)) nextValues[field] = currentValues[field];
    }
    for (const [field, value] of Object.entries(serverValues)) {
      if (!drainingFields.current.has(field) && !queuedSaves.current.has(field)) {
        canonicalValues.current[field] = value;
      }
    }
    form.reset(nextValues, { keepDirtyValues: true, keepErrors: true });
  }, [form, serverValues]);

  useEffect(() => {
    // React Strict Mode replays effects in development, so each setup must
    // restore the mounted flag after its matching cleanup.
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refreshInOrder = () => {
    const refresh = refreshTail.current.then(refreshConfig);
    refreshTail.current = refresh.then(
      () => undefined,
      () => undefined,
    );
    return refresh;
  };

  const drainField = (field: string): Promise<void> => {
    const existing = drainPromises.current.get(field);
    if (existing) return existing;

    drainingFields.current.add(field);
    if (mounted.current) setPendingFields((current) => new Set(current).add(field));

    const drain = (async () => {
      try {
        let queued = queuedSaves.current.get(field);
        while (queued) {
          queuedSaves.current.delete(field);
          const { generation, value, draftValue } = queued;

          try {
            const outcome = await serializeConfigSave(async () => {
              const patch = await saveConfig.mutateAsync({
                [field]: value,
              } as Partial<AdminConfigFlat>);
              // A newer local value already supersedes this response. Persist it
              // immediately and avoid a full-config refresh that can only be stale.
              if ((saveGenerations.current[field] ?? 0) !== generation) {
                return 'superseded' as const;
              }

              saveFailures.current.delete(field);
              if (patch.activation === 'pending') {
                // §6.1: 202 means the write is durable but not yet active on
                // every process — refetch, never resubmit. The submitted value
                // is the durable baseline, so keep the draft (a refetched
                // canonical could still show the not-yet-active old value) and
                // skip the secure_path redirect until full activation.
                canonicalValues.current[field] = value;
                await refreshInOrder();
                if (
                  mounted.current &&
                  (saveGenerations.current[field] ?? 0) === generation &&
                  !queuedSaves.current.has(field)
                ) {
                  toast.success(
                    t(($) => $.admin.config.save_success),
                    {
                      description: t(($) => $.admin.config.save_pending_desc),
                    },
                  );
                }
                return 'applied' as const;
              }
              if (field === 'secure_path') {
                replaceAdminSecurePath(toText(value));
                return 'redirected' as const;
              }

              const refreshed = await refreshInOrder();
              if (
                mounted.current &&
                (saveGenerations.current[field] ?? 0) === generation &&
                !queuedSaves.current.has(field)
              ) {
                const canonicalValue = getConfigSectionValues(refreshed, group, t)[field];
                canonicalValues.current[field] = canonicalValue;
                // Do not overwrite text entered while this request was in flight.
                // Its eventual blur will enqueue a new revision against the now
                // current canonical value.
                if (configValuesEqual(form.getValues(field), draftValue)) {
                  form.resetField(field, { defaultValue: canonicalValue });
                }
                toast.success(t(($) => $.admin.config.save_success));
              }
              return 'applied' as const;
            });
            if (outcome === 'superseded') {
              queued = queuedSaves.current.get(field);
              continue;
            }
            if (outcome === 'redirected') {
              return;
            }
          } catch (error) {
            // A failed superseded request must not block or annotate the newer
            // value waiting behind it. Only the queue tail owns inline feedback.
            if (
              mounted.current &&
              (saveGenerations.current[field] ?? 0) === generation &&
              !queuedSaves.current.has(field)
            ) {
              saveFailures.current.set(field, { generation, error });
              form.setError(field, {
                type: 'server',
                message: getErrorPresentation(error).message,
              });
              if (hasProblemCode(error, 'config_revision_conflict')) {
                // §6.1: a stale-revision write must refetch the winning
                // config — never blindly resubmit the losing value. The
                // conflicted field keeps its draft plus the inline error.
                try {
                  await refreshInOrder();
                } catch {
                  // The refetch failure surfaces through the config query.
                }
              }
            }
          }

          queued = queuedSaves.current.get(field);
        }
      } finally {
        drainingFields.current.delete(field);
        drainPromises.current.delete(field);
        if (mounted.current) {
          setPendingFields((current) => {
            const next = new Set(current);
            next.delete(field);
            return next;
          });
        }
      }
    })();

    drainPromises.current.set(field, drain);
    return drain;
  };

  const saveField = (
    requestedGroup: ConfigGroupKey,
    field: string,
    value: ConfigFieldValue,
  ): Promise<void> => {
    const allowedFields: readonly string[] = SECTION_FIELDS[group];
    if (requestedGroup !== group || !allowedFields.includes(field)) {
      throw new Error(t(($) => $.admin.config.field_not_in_section, { field, group }));
    }

    const payloadValue = configFieldValueSchema.safeParse(value);
    const fieldValue = SECTION_SCHEMAS[group].safeParse({ [field]: value });
    if (!payloadValue.success || !fieldValue.success) {
      form.setError(field, { type: 'validate', message: t(($) => $.admin.config.invalid_value) });
      return Promise.resolve();
    }
    if (field === 'secure_path' && toText(payloadValue.data).trim() === '') {
      form.setError(field, {
        type: 'validate',
        message: t(($) => $.admin.config.secure_path_required),
      });
      return Promise.resolve();
    }

    if (
      !queuedSaves.current.has(field) &&
      !drainingFields.current.has(field) &&
      configValuesEqual(payloadValue.data, canonicalValues.current[field])
    ) {
      saveFailures.current.delete(field);
      form.clearErrors(field);
      return Promise.resolve();
    }

    const generation = (saveGenerations.current[field] ?? 0) + 1;
    saveGenerations.current[field] = generation;
    queuedSaves.current.set(field, {
      generation,
      value: payloadValue.data,
      draftValue: form.getValues(field),
    });
    saveFailures.current.delete(field);
    form.clearErrors(field);
    return drainField(field);
  };

  const flushFields = async (fields: readonly string[]) => {
    for (const field of fields) {
      if (
        form.getFieldState(field).isDirty &&
        !queuedSaves.current.has(field) &&
        !drainingFields.current.has(field)
      ) {
        await saveField(group, field, form.getValues(field));
      }
    }

    while (true) {
      const pending = fields
        .map((field) => drainPromises.current.get(field))
        .filter((promise): promise is Promise<void> => Boolean(promise));
      if (pending.length === 0) break;
      await Promise.all(pending);
    }

    return !fields.some((field) => saveFailures.current.has(field));
  };

  const runTestMail = async () => {
    setActionPending(true);
    try {
      if (await flushFields(SECTION_FIELDS.email)) await onTestMail();
    } catch {
      // Mutation failures are presented by the shared query error boundary.
    } finally {
      if (mounted.current) setActionPending(false);
    }
  };

  const runSetWebhook = async () => {
    setActionPending(true);
    try {
      if (!(await flushFields(['telegram_bot_token']))) return;
      await onSetWebhook(toText(form.getValues('telegram_bot_token')));
    } catch {
      // Mutation failures are presented by the shared query error boundary.
    } finally {
      if (mounted.current) setActionPending(false);
    }
  };

  const ctx: FormCtx = {
    control: form.control,
    get: (requestedGroup, field) => (requestedGroup === group ? values[field] : undefined),
    isSaving: (field) => pendingFields.has(field),
    save: (requestedGroup, field, value) => {
      return saveField(requestedGroup, field, value);
    },
  };

  switch (group) {
    case 'site':
      if (!plans) throw new Error(t(($) => $.admin.config.site_plans_missing));
      return <SiteSection ctx={ctx} plans={plans} />;
    case 'safe':
      return <SafeSection ctx={ctx} />;
    case 'subscribe':
      return <SubscribeSection ctx={ctx} />;
    case 'deposit':
      return <DepositSection ctx={ctx} />;
    case 'ticket':
      return <TicketSection ctx={ctx} />;
    case 'invite':
      return <InviteSection ctx={ctx} />;
    case 'frontend':
      return <FrontendSection ctx={ctx} />;
    case 'server':
      return <ServerSection ctx={ctx} />;
    case 'email':
      if (!emailTemplates) throw new Error(t(($) => $.admin.config.email_templates_missing));
      return (
        <EmailSection
          ctx={ctx}
          templates={emailTemplates}
          onTest={() => void runTestMail()}
          testing={testMailPending || actionPending}
        />
      );
    case 'telegram':
      return (
        <TelegramSection
          ctx={ctx}
          onWebhook={() => void runSetWebhook()}
          webhookPending={webhookPending || actionPending}
        />
      );
    case 'app':
      return <AppSection ctx={ctx} />;
  }
}
