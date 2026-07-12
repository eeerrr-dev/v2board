import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useWatch, type Control } from 'react-hook-form';
import { z } from 'zod';
import { Loader2 } from 'lucide-react';
import type { AdminConfig, AdminConfigFlat, AdminConfigGroups, Plan } from '@v2board/types';
import { getErrorPresentation } from '@v2board/api-client';
import {
  useAdminPlans,
  useConfig,
  useEmailTemplates,
  useSaveSystemConfigMutation,
  useSetTelegramWebhookMutation,
  useTestSendMailMutation,
} from '@/lib/queries';
import { cn } from '@/lib/cn';
import { toast } from '@/lib/toast';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Field, FieldError } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';

type ConfigGroupKey = keyof AdminConfigGroups;

type ConfigFieldValue = string | number | string[] | null | undefined;
type ConfigSectionValues = Record<string, ConfigFieldValue>;

const SECTION_FIELDS = {
  site: [
    'app_name',
    'app_description',
    'app_url',
    'force_https',
    'logo',
    'subscribe_url',
    'subscribe_path',
    'tos_url',
    'stop_register',
    'try_out_plan_id',
    'try_out_hour',
    'currency',
    'currency_symbol',
  ],
  safe: [
    'email_verify',
    'email_gmail_limit_enable',
    'safe_mode_enable',
    'secure_path',
    'email_whitelist_enable',
    'email_whitelist_suffix',
    'recaptcha_enable',
    'recaptcha_key',
    'recaptcha_site_key',
    'register_limit_by_ip_enable',
    'register_limit_count',
    'register_limit_expire',
    'password_limit_enable',
    'password_limit_count',
    'password_limit_expire',
  ],
  subscribe: [
    'plan_change_enable',
    'reset_traffic_method',
    'surplus_enable',
    'allow_new_period',
    'new_order_event_id',
    'renew_order_event_id',
    'change_order_event_id',
    'show_info_to_server_enable',
    'show_subscribe_method',
    'show_subscribe_expire',
  ],
  deposit: ['deposit_bounus'],
  ticket: ['ticket_status'],
  invite: [
    'invite_force',
    'invite_commission',
    'invite_gen_limit',
    'invite_never_expire',
    'commission_first_time_enable',
    'commission_auto_check_enable',
    'commission_withdraw_limit',
    'commission_withdraw_method',
    'withdraw_close_enable',
    'commission_distribution_enable',
    'commission_distribution_l1',
    'commission_distribution_l2',
    'commission_distribution_l3',
  ],
  frontend: ['frontend_theme_color', 'frontend_background_url', 'frontend_custom_html'],
  server: [
    'server_api_url',
    'server_token',
    'server_pull_interval',
    'server_push_interval',
    'server_node_report_min_traffic',
    'server_device_online_min_traffic',
    'device_limit_mode',
  ],
  email: [
    'email_host',
    'email_port',
    'email_encryption',
    'email_username',
    'email_password',
    'email_from_address',
    'email_template',
  ],
  telegram: ['telegram_bot_token', 'telegram_bot_enable', 'telegram_discuss_link'],
  app: [
    'windows_version',
    'windows_download_url',
    'macos_version',
    'macos_download_url',
    'android_version',
    'android_download_url',
  ],
} as const satisfies Record<ConfigGroupKey, readonly string[]>;

const configFieldValueSchema = z.union([
  z.string(),
  z.number(),
  z.array(z.string()),
  z.null(),
  z.undefined(),
]);

function createSectionSchema(
  fields: readonly string[],
): z.ZodType<ConfigSectionValues, ConfigSectionValues> {
  const allowed = new Set(fields);
  return z.record(z.string(), configFieldValueSchema).superRefine((values, ctx) => {
    for (const field of Object.keys(values)) {
      if (allowed.has(field)) continue;
      ctx.addIssue({ code: 'custom', path: [field], message: '配置字段不属于当前分组' });
    }
  });
}

const SECTION_SCHEMAS: Record<
  ConfigGroupKey,
  z.ZodType<ConfigSectionValues, ConfigSectionValues>
> = {
  site: createSectionSchema(SECTION_FIELDS.site),
  safe: createSectionSchema(SECTION_FIELDS.safe),
  subscribe: createSectionSchema(SECTION_FIELDS.subscribe),
  deposit: createSectionSchema(SECTION_FIELDS.deposit),
  ticket: createSectionSchema(SECTION_FIELDS.ticket),
  invite: createSectionSchema(SECTION_FIELDS.invite),
  frontend: createSectionSchema(SECTION_FIELDS.frontend),
  server: createSectionSchema(SECTION_FIELDS.server),
  email: createSectionSchema(SECTION_FIELDS.email),
  telegram: createSectionSchema(SECTION_FIELDS.telegram),
  app: createSectionSchema(SECTION_FIELDS.app),
};

const SECTIONS: { key: ConfigGroupKey; title: string }[] = [
  { key: 'site', title: '站点' },
  { key: 'safe', title: '安全' },
  { key: 'subscribe', title: '订阅' },
  { key: 'deposit', title: '充值' },
  { key: 'ticket', title: '工单' },
  { key: 'invite', title: '邀请&佣金' },
  { key: 'frontend', title: '个性化' },
  { key: 'server', title: '节点' },
  { key: 'email', title: '邮件' },
  { key: 'telegram', title: 'Telegram' },
  { key: 'app', title: 'APP' },
];

export default function ConfigPage() {
  return <SystemConfigPage />;
}

// ---------------------------------------------------------------------------
// System config (grouped setting fields, auto-saved per field to /config/save)
// ---------------------------------------------------------------------------

interface FormCtx {
  control: Control<ConfigSectionValues>;
  get: (group: ConfigGroupKey, field: string) => ConfigFieldValue;
  isSaving: (field: string) => boolean;
  save: (group: ConfigGroupKey, field: string, value: ConfigFieldValue) => void;
}

function SystemConfigPage() {
  const config = useConfig();
  const plans = useAdminPlans();
  const emailTemplates = useEmailTemplates();
  const webhook = useSetTelegramWebhookMutation();
  const testMail = useTestSendMailMutation();
  const [active, setActive] = useState<ConfigGroupKey>('site');

  const sendTestMail = () => {
    testMail.mutate(undefined, {
      onSuccess: (result) => {
        const log = result.log;
        if (log?.error) {
          toast.error('发送失败', { description: log.error });
        } else {
          toast.success('发送成功', { description: `收信地址：${log?.email ?? ''}` });
        }
      },
    });
  };

  const setWebhook = () => {
    webhook.mutate(undefined, {
      onSuccess: () => toast.success('webhook 设置成功'),
    });
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

      <div className="grid gap-6 lg:grid-cols-[180px_1fr] lg:items-start">
        <nav
          className="flex flex-row flex-wrap gap-1 lg:sticky lg:top-4 lg:flex-col"
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

function getConfigSectionValues(config: AdminConfig, group: ConfigGroupKey): ConfigSectionValues {
  const source = config[group];
  if (!source || typeof source !== 'object' || Array.isArray(source)) return {};

  const record = Object.fromEntries(Object.entries(source));
  const selected: Record<string, unknown> = {};
  for (const field of SECTION_FIELDS[group]) {
    if (Object.hasOwn(record, field)) selected[field] = record[field];
  }

  const parsed = SECTION_SCHEMAS[group].safeParse(selected);
  if (!parsed.success) {
    throw new Error(`系统配置「${group}」的数据格式不正确`, { cause: parsed.error });
  }
  return parsed.data;
}

function SystemConfigSectionForm({
  group,
  config,
  plans,
  emailTemplates,
  onTestMail,
  testMailPending,
  onSetWebhook,
  webhookPending,
  refreshConfig,
}: {
  group: ConfigGroupKey;
  config: AdminConfig;
  plans?: Plan[];
  emailTemplates?: string[];
  onTestMail: () => void;
  testMailPending: boolean;
  onSetWebhook: () => void;
  webhookPending: boolean;
  refreshConfig: () => Promise<AdminConfig>;
}) {
  const saveConfig = useSaveSystemConfigMutation();
  const serverValues = useMemo(() => getConfigSectionValues(config, group), [config, group]);
  const form = useForm<ConfigSectionValues>({
    resolver: zodResolver(SECTION_SCHEMAS[group]),
    defaultValues: serverValues,
    mode: 'onChange',
  });
  const values = useWatch({ control: form.control });
  const [pendingFields, setPendingFields] = useState<ReadonlySet<string>>(() => new Set());
  const queuedSaves = useRef(new Map<string, { generation: number; value: ConfigFieldValue }>());
  const drainingFields = useRef(new Set<string>());
  const saveGenerations = useRef<Record<string, number>>({});
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

  const drainField = async (field: string) => {
    if (drainingFields.current.has(field)) return;
    drainingFields.current.add(field);
    if (mounted.current) setPendingFields((current) => new Set(current).add(field));

    try {
      let queued = queuedSaves.current.get(field);
      while (queued) {
        queuedSaves.current.delete(field);
        const { generation, value } = queued;

        try {
          await saveConfig.mutateAsync({ [field]: value } as Partial<AdminConfigFlat>);
          // A newer local value already supersedes this response. Persist it
          // immediately and avoid a full-config refresh that can only be stale.
          if ((saveGenerations.current[field] ?? 0) !== generation) {
            queued = queuedSaves.current.get(field);
            continue;
          }

          const refreshed = await refreshInOrder();
          if (
            mounted.current &&
            (saveGenerations.current[field] ?? 0) === generation &&
            !queuedSaves.current.has(field)
          ) {
            const canonicalValue = getConfigSectionValues(refreshed, group)[field];
            form.resetField(field, { defaultValue: canonicalValue });
            toast.success('保存成功');
          }
        } catch (error) {
          // A failed superseded request must not block or annotate the newer
          // value waiting behind it. Only the queue tail owns inline feedback.
          if (
            mounted.current &&
            (saveGenerations.current[field] ?? 0) === generation &&
            !queuedSaves.current.has(field)
          ) {
            form.setError(field, {
              type: 'server',
              message: getErrorPresentation(error).message,
            });
          }
        }

        queued = queuedSaves.current.get(field);
      }
    } finally {
      drainingFields.current.delete(field);
      if (mounted.current) {
        setPendingFields((current) => {
          const next = new Set(current);
          next.delete(field);
          return next;
        });
      }
    }
  };

  const saveField = (requestedGroup: ConfigGroupKey, field: string, value: ConfigFieldValue) => {
    const allowedFields: readonly string[] = SECTION_FIELDS[group];
    if (requestedGroup !== group || !allowedFields.includes(field)) {
      throw new Error(`配置字段「${field}」不属于「${group}」分组`);
    }

    const payloadValue = configFieldValueSchema.safeParse(value);
    const fieldValue = SECTION_SCHEMAS[group].safeParse({ [field]: value });
    if (!payloadValue.success || !fieldValue.success) {
      form.setError(field, { type: 'validate', message: '请输入有效的配置值' });
      return;
    }

    const generation = (saveGenerations.current[field] ?? 0) + 1;
    saveGenerations.current[field] = generation;
    queuedSaves.current.set(field, { generation, value: payloadValue.data });
    form.clearErrors(field);
    void drainField(field);
  };

  const ctx: FormCtx = {
    control: form.control,
    get: (requestedGroup, field) => (requestedGroup === group ? values[field] : undefined),
    isSaving: (field) => pendingFields.has(field),
    save: (requestedGroup, field, value) => {
      saveField(requestedGroup, field, value);
    },
  };

  switch (group) {
    case 'site':
      if (!plans) throw new Error('站点配置缺少订阅依赖');
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
      if (!emailTemplates) throw new Error('邮件配置缺少模板依赖');
      return (
        <EmailSection
          ctx={ctx}
          templates={emailTemplates}
          onTest={onTestMail}
          testing={testMailPending}
        />
      );
    case 'telegram':
      return <TelegramSection ctx={ctx} onWebhook={onSetWebhook} webhookPending={webhookPending} />;
    case 'app':
      return <AppSection ctx={ctx} />;
  }
}

// --- Shared field primitives ----------------------------------------------

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="divide-y divide-border">{children}</CardContent>
    </Card>
  );
}

function SettingRow({
  title,
  description,
  indent,
  children,
}: {
  title: string;
  description?: string;
  indent?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        'flex flex-col gap-2 py-4 sm:flex-row sm:items-start sm:justify-between sm:gap-6',
        indent && 'sm:pl-6',
      )}
    >
      <div className="space-y-1 sm:max-w-md">
        <div className="text-sm font-medium text-foreground">{title}</div>
        {description ? (
          <p className="text-xs leading-5 text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="w-full sm:w-72 sm:shrink-0">{children}</div>
    </div>
  );
}

function SwitchRow({
  ctx,
  group,
  field,
  title,
  description,
  indent,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  indent?: boolean;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => (
        <SettingRow title={title} description={description} indent={indent}>
          <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
            <div className="flex h-10 items-center sm:justify-end">
              <Switch
                ref={controlField.ref}
                name={controlField.name}
                checked={isBackendEnabled(controlField.value)}
                onBlur={controlField.onBlur}
                onCheckedChange={(checked) => {
                  const value = checked ? 1 : 0;
                  controlField.onChange(value);
                  ctx.save(group, field, value);
                }}
                aria-label={title}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${field}`}
              />
            </div>
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

function TextRow({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  type,
  suffix,
  indent,
  coerce,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  placeholder?: string;
  type?: string;
  suffix?: string;
  indent?: boolean;
  coerce?: (value: string) => ConfigFieldValue;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => (
        <SettingRow title={title} description={description} indent={indent}>
          <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
            <div className={suffix ? 'relative' : undefined}>
              <Input
                ref={controlField.ref}
                name={controlField.name}
                type={type}
                className={suffix ? 'pr-10' : undefined}
                placeholder={placeholder}
                aria-label={title}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${field}`}
                value={toText(controlField.value)}
                onChange={(event) => controlField.onChange(event.target.value)}
                onBlur={(event) => {
                  controlField.onBlur();
                  ctx.save(group, field, coerce ? coerce(event.target.value) : event.target.value);
                }}
              />
              {suffix ? (
                <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                  {suffix}
                </span>
              ) : null}
            </div>
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

function TextareaRow({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  rows,
  indent,
  coerce,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  placeholder?: string;
  rows: number;
  indent?: boolean;
  coerce?: (value: string) => ConfigFieldValue;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => (
        <SettingRow title={title} description={description} indent={indent}>
          <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
            <Textarea
              ref={controlField.ref}
              name={controlField.name}
              rows={rows}
              placeholder={placeholder}
              aria-label={title}
              aria-invalid={fieldState.invalid}
              data-testid={`config-${field}`}
              value={toText(controlField.value)}
              onChange={(event) => controlField.onChange(event.target.value)}
              onBlur={(event) => {
                controlField.onBlur();
                ctx.save(group, field, coerce ? coerce(event.target.value) : event.target.value);
              }}
            />
            <FieldError errors={[fieldState.error]} />
          </Field>
        </SettingRow>
      )}
    />
  );
}

function SelectRow({
  ctx,
  group,
  field,
  title,
  description,
  placeholder,
  options,
  fallback,
  indent,
}: {
  ctx: FormCtx;
  group: ConfigGroupKey;
  field: string;
  title: string;
  description?: string;
  placeholder?: string;
  options: { value: string; label: string }[];
  fallback?: string;
  indent?: boolean;
}) {
  return (
    <Controller
      control={ctx.control}
      name={field}
      render={({ field: controlField, fieldState }) => {
        const current =
          controlField.value == null || controlField.value === ''
            ? fallback
            : String(controlField.value);
        return (
          <SettingRow title={title} description={description} indent={indent}>
            <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(field)}>
              <Select
                name={controlField.name}
                value={current}
                onValueChange={(value) => {
                  controlField.onChange(value);
                  ctx.save(group, field, value);
                }}
              >
                <SelectTrigger
                  ref={controlField.ref}
                  className="w-full"
                  aria-label={title}
                  aria-invalid={fieldState.invalid}
                  data-testid={`config-${field}`}
                >
                  <SelectValue placeholder={placeholder} />
                </SelectTrigger>
                <SelectContent>
                  {options.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <FieldError errors={[fieldState.error]} />
            </Field>
          </SettingRow>
        );
      }}
    />
  );
}

const ORDER_EVENT_OPTIONS = [
  { value: '0', label: '不执行任何动作' },
  { value: '1', label: '重置用户流量' },
];

function WarningAlert({ children }: { children: ReactNode }) {
  return (
    <Alert className="border-warning/30 bg-warning/10 text-warning">
      <AlertDescription className="text-warning">{children}</AlertDescription>
    </Alert>
  );
}

// --- Sections --------------------------------------------------------------

function SiteSection({ ctx, plans }: { ctx: FormCtx; plans: Plan[] }) {
  const tryOutOff = String(ctx.get('site', 'try_out_plan_id') ?? 0) === '0';
  return (
    <Section title="站点">
      <TextRow
        ctx={ctx}
        group="site"
        field="app_name"
        title="站点名称"
        description="用于显示需要站点名称的地方。"
        placeholder="请输入站点名称"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="app_description"
        title="站点描述"
        description="用于显示需要站点描述的地方。"
        placeholder="请输入站点描述"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="app_url"
        title="站点网址"
        description="当前网站最新网址，将会在邮件等需要用于网址处体现。"
        placeholder="请输入站点URL，末尾不要/"
      />
      <SwitchRow
        ctx={ctx}
        group="site"
        field="force_https"
        title="强制HTTPS"
        description="当站点没有使用HTTPS，CDN或反代开启强制HTTPS时需要开启。"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="logo"
        title="LOGO"
        description="用于显示需要LOGO的地方。"
        placeholder="请输入LOGO URL，末尾不要/"
      />
      <TextareaRow
        ctx={ctx}
        group="site"
        field="subscribe_url"
        title="订阅URL"
        description="用于订阅所使用，留空则为站点URL。如需多个订阅URL随机获取请使用逗号进行分割。"
        placeholder="请输入订阅URL，末尾不要/。逗号分割支持多域名"
        rows={4}
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="subscribe_path"
        title="订阅路径"
        description="用于订阅所使用，留空则为/api/v1/client/subscribe。如需更换不同的订阅路径请设置。"
        placeholder="/api/v1/client/subscribe"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="tos_url"
        title="用户条款(TOS)URL"
        description="用于跳转到用户条款(TOS)"
        placeholder="请输入用户条款URL，末尾不要/"
      />
      <SwitchRow
        ctx={ctx}
        group="site"
        field="stop_register"
        title="停止新用户注册"
        description="开启后任何人都将无法进行注册。"
      />
      <SelectRow
        ctx={ctx}
        group="site"
        field="try_out_plan_id"
        title="注册试用"
        description="选择需要试用的订阅，如果没有选项请先前往订阅管理添加。"
        placeholder="请选择试用订阅"
        fallback="0"
        options={[
          { value: '0', label: '关闭' },
          ...plans.map((plan) => ({ value: String(plan.id), label: plan.name })),
        ]}
      />
      {tryOutOff ? null : (
        <TextRow
          ctx={ctx}
          group="site"
          field="try_out_hour"
          title="试用时间(小时)"
          placeholder="请输入"
          indent
        />
      )}
      <TextRow
        ctx={ctx}
        group="site"
        field="currency"
        title="货币单位"
        description="仅用于展示使用，更改后系统中所有的货币单位都将发生变更。"
        placeholder="CNY"
      />
      <TextRow
        ctx={ctx}
        group="site"
        field="currency_symbol"
        title="货币符号"
        description="仅用于展示使用，更改后系统中所有的货币单位都将发生变更。"
        placeholder="¥"
      />
    </Section>
  );
}

function SafeSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="安全">
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_verify"
        title="邮箱验证"
        description="开启后将会强制要求用户进行邮箱验证。"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_gmail_limit_enable"
        title="禁止使用Gmail多别名"
        description="开启后Gmail多别名将无法注册。"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="safe_mode_enable"
        title="安全模式"
        description="开启后除了站点URL以外的绑定本站点的域名访问都将会被403。"
      />
      <TextRow
        ctx={ctx}
        group="safe"
        field="secure_path"
        title="后台路径"
        description="后台管理路径，修改后将会改变原有的admin路径"
        placeholder="admin"
      />
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="email_whitelist_enable"
        title="邮箱后缀白名单"
        description="开启后在名单中的邮箱后缀才允许进行注册。"
      />
      {isBackendEnabled(ctx.get('safe', 'email_whitelist_enable')) ? (
        <TextareaRow
          ctx={ctx}
          group="safe"
          field="email_whitelist_suffix"
          title="白名单后缀"
          description="请使用逗号进行分割，如：qq.com,gmail.com。"
          placeholder="请输入后缀域名，逗号分割 如：qq.com,gmail.com"
          rows={4}
          indent
          coerce={splitComma}
        />
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="recaptcha_enable"
        title="防机器人"
        description="开启后将会使用Google reCAPTCHA防止机器人。"
      />
      {isBackendEnabled(ctx.get('safe', 'recaptcha_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="recaptcha_key"
            title="密钥"
            description="在Google reCAPTCHA申请的密钥。"
            placeholder="请输入"
            indent
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="recaptcha_site_key"
            title="网站密钥"
            description="在Google reCAPTCH申请的网站密钥。"
            placeholder="请输入"
            indent
          />
        </>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="register_limit_by_ip_enable"
        title="IP注册限制"
        description="开启后如果IP注册账户达到规则要求将会被限制注册，请注意IP判断可能因为CDN或前置代理导致问题。"
      />
      {isBackendEnabled(ctx.get('safe', 'register_limit_by_ip_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="register_limit_count"
            title="次数"
            description="达到注册次数后开启惩罚。"
            placeholder="请输入"
            indent
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="register_limit_expire"
            title="惩罚时间(分钟)"
            description="需要等待惩罚时间过后才可以再次注册。"
            placeholder="请输入"
            indent
          />
        </>
      ) : null}
      <SwitchRow
        ctx={ctx}
        group="safe"
        field="password_limit_enable"
        title="防爆破限制"
        description="开启后如果该账户尝试登陆失败次数过多将会被限制。"
      />
      {isBackendEnabled(ctx.get('safe', 'password_limit_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="safe"
            field="password_limit_count"
            title="次数"
            description="达到失败次数后开启惩罚。"
            placeholder="请输入"
            indent
          />
          <TextRow
            ctx={ctx}
            group="safe"
            field="password_limit_expire"
            title="惩罚时间(分钟)"
            description="需要等待惩罚时间过后才可以再次登陆。"
            placeholder="请输入"
            indent
          />
        </>
      ) : null}
    </Section>
  );
}

function SubscribeSection({ ctx }: { ctx: FormCtx }) {
  const timedExpire = String(ctx.get('subscribe', 'show_subscribe_method') ?? 0) === '2';
  return (
    <Section title="订阅">
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="plan_change_enable"
        title="允许用户更改订阅"
        description="开启后用户将会可以对订阅计划进行变更。"
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="reset_traffic_method"
        title="月流量重置方式"
        description="全局流量重置方式，默认每月1号。可以在订阅管理为订阅单独设置。"
        placeholder="请选择订阅重置方式"
        fallback="0"
        options={[
          { value: '0', label: '每月1号' },
          { value: '1', label: '按月重置' },
          { value: '2', label: '不重置' },
          { value: '3', label: '每年1月1日' },
          { value: '4', label: '按年重置' },
        ]}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="surplus_enable"
        title="开启折抵方案"
        description="开启后用户更换订阅将会由系统对原有订阅进行折抵，方案参考文档。"
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="allow_new_period"
        title="允许提前开启流量周期"
        description="开启后用户流量用尽时可以选择扣除订阅时长为代价重置流量，按月重置时扣除本周期剩余订阅时长，每月1号重置时扣除整月时间30天。"
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="new_order_event_id"
        title="当订阅新购时触发事件"
        description="新购订阅完成时将触发该任务。"
        placeholder="请选择事件"
        fallback="0"
        options={ORDER_EVENT_OPTIONS}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="renew_order_event_id"
        title="当订阅续费时触发事件"
        description="续费订阅完成时将触发该任务。"
        placeholder="请选择事件"
        fallback="0"
        options={ORDER_EVENT_OPTIONS}
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="change_order_event_id"
        title="当订阅变更时触发事件"
        description="变更订阅完成时将触发该任务。"
        placeholder="请选择事件"
        fallback="0"
        options={ORDER_EVENT_OPTIONS}
      />
      <SwitchRow
        ctx={ctx}
        group="subscribe"
        field="show_info_to_server_enable"
        title="在订阅中展示订阅信息"
        description="开启后将会在用户订阅节点时输出订阅信息。"
      />
      <SelectRow
        ctx={ctx}
        group="subscribe"
        field="show_subscribe_method"
        title="订阅链接生效模式"
        description="用户获取订阅链接后的有效期。"
        placeholder="请选择"
        fallback="0"
        options={[
          { value: '0', label: '永久有效' },
          { value: '1', label: '一次性有效' },
          { value: '2', label: '限时有效' },
        ]}
      />
      {timedExpire ? (
        <TextRow
          ctx={ctx}
          group="subscribe"
          field="show_subscribe_expire"
          title="订阅链接有效时间(分钟)"
          description="订阅链接获取后经过该时间将失效。"
          placeholder="请输入"
          indent
        />
      ) : null}
    </Section>
  );
}

function DepositSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="充值">
      <TextareaRow
        ctx={ctx}
        group="deposit"
        field="deposit_bounus"
        title="充值奖励"
        description="充值一定金额可以获得的奖励。"
        placeholder={'请输入 充值金额:奖励金额,逗号分割\n如 50:18,100:38, 200:88'}
        rows={2}
        coerce={splitComma}
      />
    </Section>
  );
}

function TicketSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="工单">
      <SelectRow
        ctx={ctx}
        group="ticket"
        field="ticket_status"
        title="工单设置"
        description="请选择工单的状态。"
        fallback="0"
        options={[
          { value: '0', label: '完全开放工单' },
          { value: '1', label: '仅限有付费订单用户' },
          { value: '2', label: '完全禁止工单' },
        ]}
      />
    </Section>
  );
}

function InviteSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="邀请&佣金">
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="invite_force"
        title="开启强制邀请"
        description="开启后只有被邀请的用户才可以进行注册。"
      />
      <TextRow
        ctx={ctx}
        group="invite"
        field="invite_commission"
        title="邀请佣金百分比"
        description="默认全局的佣金分配比例，你可以在用户管理单独配置单个比例。"
        placeholder="请输入"
        coerce={parseBackendInteger}
      />
      <TextRow
        ctx={ctx}
        group="invite"
        field="invite_gen_limit"
        title="用户可创建邀请码上限"
        placeholder="请输入"
        coerce={parseBackendInteger}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="invite_never_expire"
        title="邀请码永不失效"
        description="开启后邀请码被使用后将不会失效，否则使用过后即失效。"
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="commission_first_time_enable"
        title="佣金仅首次发放"
        description="开启后被邀请人首次支付时才会产生佣金，可以在用户管理对用户进行单独配置。"
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="commission_auto_check_enable"
        title="佣金自动确认"
        description="开启后佣金将会在订单完成3日后自动进行确认。"
      />
      <TextRow
        ctx={ctx}
        group="invite"
        field="commission_withdraw_limit"
        title="提现单申请门槛(元)"
        description="小于门槛金额的提现单将不会被提交。"
        placeholder="请输入"
      />
      <TextareaRow
        ctx={ctx}
        group="invite"
        field="commission_withdraw_method"
        title="提现方式"
        description="可以支持的提现方式。"
        placeholder="请输入后缀域名，逗号分割 如：支付宝,USDT,贝宝"
        rows={4}
        coerce={splitComma}
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="withdraw_close_enable"
        title="关闭提现"
        description="关闭后将禁止用户申请提现，且邀请佣金将会直接进入用户余额。"
      />
      <SwitchRow
        ctx={ctx}
        group="invite"
        field="commission_distribution_enable"
        title="三级分销"
        description="开启后将佣金将按照设置的3成比例进行分成，三成比例合计请不要>100%。"
      />
      {isBackendEnabled(ctx.get('invite', 'commission_distribution_enable')) ? (
        <>
          <TextRow
            ctx={ctx}
            group="invite"
            field="commission_distribution_l1"
            title="一级邀请人比例"
            placeholder="请输入比例如：50"
            indent
          />
          <TextRow
            ctx={ctx}
            group="invite"
            field="commission_distribution_l2"
            title="二级邀请人比例"
            placeholder="请输入比例如：30"
            indent
          />
          <TextRow
            ctx={ctx}
            group="invite"
            field="commission_distribution_l3"
            title="三级邀请人比例"
            placeholder="请输入比例如：20"
            indent
          />
        </>
      ) : null}
    </Section>
  );
}

function FrontendSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="个性化">
      <SelectRow
        ctx={ctx}
        group="frontend"
        field="frontend_theme_color"
        title="主题色"
        fallback="default"
        options={[
          { value: 'default', label: '默认' },
          { value: 'black', label: '黑色' },
          { value: 'darkblue', label: '暗蓝色' },
          { value: 'green', label: '奶绿色' },
        ]}
      />
      <TextRow
        ctx={ctx}
        group="frontend"
        field="frontend_background_url"
        title="背景"
        description="将会在后台登录页面进行展示。"
        placeholder="https://xxxxx.com/wallpaper.png"
      />
      <TextareaRow
        ctx={ctx}
        group="frontend"
        field="frontend_custom_html"
        title="自定义集成 HTML"
        description="仅供可信运维人员集成统计、客服等代码；内容会原样注入用户端页面，请勿粘贴任何不可信 HTML 或脚本。"
        placeholder="<!-- 仅粘贴经过审核的可信集成代码 -->"
        rows={8}
      />
    </Section>
  );
}

function ServerSection({ ctx }: { ctx: FormCtx }) {
  return (
    <Section title="节点">
      <TextRow
        ctx={ctx}
        group="server"
        field="server_api_url"
        title="节点对接API地址"
        description="v2node节点一键对接专用地址。"
        placeholder="请输入"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_token"
        title="通讯密钥"
        description="V2board与节点通讯的密钥，以便数据不会被他人获取。"
        placeholder="请输入"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_pull_interval"
        title="节点拉取动作轮询间隔"
        description="节点从面板获取数据的间隔频率。"
        placeholder="请输入"
        type="number"
        suffix="秒"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_push_interval"
        title="节点推送动作轮询间隔"
        description="节点推送数据到面板的间隔频率。"
        placeholder="请输入"
        type="number"
        suffix="秒"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_node_report_min_traffic"
        title="节点用户流量上报最低阈值"
        description="每次推送动作仅累计使用流量高于阈值的用户信息会被上报，未上报流量会累计"
        placeholder="请输入"
        type="number"
        suffix="Kb"
      />
      <TextRow
        ctx={ctx}
        group="server"
        field="server_device_online_min_traffic"
        title="节点用户设备数统计最低阈值"
        description="每次推送动作仅上报流量高于阈值的在线设备IP地址会被节点统计"
        placeholder="请输入"
        type="number"
        suffix="Kb"
      />
      <SwitchRow
        ctx={ctx}
        group="server"
        field="device_limit_mode"
        title="全局设备数限制采用宽松模式"
        description="开启后同一IP地址使用多个节点只统计为一个设备"
      />
    </Section>
  );
}

function EmailSection({
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
  return (
    <div className="space-y-4">
      <WarningAlert>
        保存后 API 与后台任务会自动应用最新邮件配置；本页配置优先级高于环境变量中的邮件配置。
      </WarningAlert>
      <Section title="邮件">
        <TextRow
          ctx={ctx}
          group="email"
          field="email_host"
          title="SMTP服务器地址"
          description="由邮件服务商提供的服务地址"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_port"
          title="SMTP服务端口"
          description="常见的端口有25, 465, 587"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_encryption"
          title="SMTP加密方式"
          description="465端口加密方式一般为SSL，587端口加密方式一般为TLS"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_username"
          title="SMTP账号"
          description="由邮件服务商提供的账号"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_password"
          title="SMTP密码"
          description="由邮件服务商提供的密码"
          placeholder="请输入"
        />
        <TextRow
          ctx={ctx}
          group="email"
          field="email_from_address"
          title="发件地址"
          description="由邮件服务商提供的发件地址"
          placeholder="请输入"
        />
        <SelectRow
          ctx={ctx}
          group="email"
          field="email_template"
          title="邮件模板"
          description="选择当前原生运行时提供的邮件模板"
          options={templates.map((template) => ({ value: template, label: template }))}
        />
        <SettingRow title="发送测试邮件" description="邮件将会发送到当前登陆用户邮箱">
          <Button onClick={onTest} disabled={testing} data-testid="config-test-mail">
            {testing ? (
              <Loader2 className="size-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            发送测试邮件
          </Button>
        </SettingRow>
      </Section>
    </div>
  );
}

function TelegramSection({
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

function AppSection({ ctx }: { ctx: FormCtx }) {
  return (
    <div className="space-y-4">
      <WarningAlert>用于自有客户端(APP)的版本管理及更新</WarningAlert>
      <Section title="APP">
        <AppEntryRow
          ctx={ctx}
          title="Windows"
          description="Windows端版本号及下载地址"
          versionField="windows_version"
          urlField="windows_download_url"
          urlPlaceholder="https://xxxx.com/xxx.exe"
        />
        <AppEntryRow
          ctx={ctx}
          title="macOS"
          description="macOS端版本号及下载地址"
          versionField="macos_version"
          urlField="macos_download_url"
          urlPlaceholder="https://xxxx.com/xxx.dmg"
        />
        <AppEntryRow
          ctx={ctx}
          title="Android"
          description="Android端版本号及下载地址"
          versionField="android_version"
          urlField="android_download_url"
          urlPlaceholder="https://xxxx.com/xxx.apk"
        />
      </Section>
    </div>
  );
}

function AppEntryRow({
  ctx,
  title,
  description,
  versionField,
  urlField,
  urlPlaceholder,
}: {
  ctx: FormCtx;
  title: string;
  description: string;
  versionField: string;
  urlField: string;
  urlPlaceholder: string;
}) {
  return (
    <SettingRow title={title} description={description}>
      <div className="space-y-2">
        <Controller
          control={ctx.control}
          name={versionField}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(versionField)}>
              <Input
                ref={field.ref}
                name={field.name}
                placeholder="1.0.0"
                aria-label={`${title}版本号`}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${versionField}`}
                value={toText(field.value)}
                onChange={(event) => field.onChange(event.target.value)}
                onBlur={(event) => {
                  field.onBlur();
                  ctx.save('app', versionField, event.target.value);
                }}
              />
              <FieldError errors={[fieldState.error]} />
            </Field>
          )}
        />
        <Controller
          control={ctx.control}
          name={urlField}
          render={({ field, fieldState }) => (
            <Field data-invalid={fieldState.invalid} aria-busy={ctx.isSaving(urlField)}>
              <Input
                ref={field.ref}
                name={field.name}
                placeholder={urlPlaceholder}
                aria-label={`${title}下载地址`}
                aria-invalid={fieldState.invalid}
                data-testid={`config-${urlField}`}
                value={toText(field.value)}
                onChange={(event) => field.onChange(event.target.value)}
                onBlur={(event) => {
                  field.onBlur();
                  ctx.save('app', urlField, event.target.value);
                }}
              />
              <FieldError errors={[fieldState.error]} />
            </Field>
          )}
        />
      </div>
    </SettingRow>
  );
}

// --- Backend-contract value coercions --------------------------------------

function toText(value: unknown) {
  if (Array.isArray(value)) return value.join(',');
  return value == null ? '' : String(value);
}

function splitComma(value: string) {
  return value.split(',');
}

export function parseBackendInteger(value: string) {
  return parseInt(value);
}

export function isBackendEnabled(value: unknown) {
  return Boolean(parseInt(toText(value)));
}
