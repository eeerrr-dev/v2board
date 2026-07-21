import { z } from 'zod';
import type { TFunction } from 'i18next';
import type { Control } from 'react-hook-form';
import type { AdminConfigFlat, AdminConfigGroups } from '@v2board/types';
import { parseBackendInteger, parseBackendNumber, splitComma } from './values';

export type ConfigGroupKey = keyof AdminConfigGroups;
export type ConfigFieldName = Extract<keyof AdminConfigFlat, string>;
export type ConfigGroupField<Group extends ConfigGroupKey> = Extract<
  keyof AdminConfigGroups[Group],
  ConfigFieldName
>;
export type ConfigGroupFieldWithValue<Group extends ConfigGroupKey, AcceptedValue> = {
  [Field in ConfigGroupField<Group>]: NonNullable<
    AdminConfigGroups[Group][Field]
  > extends AcceptedValue
    ? Field
    : never;
}[ConfigGroupField<Group>];
type ConfigDraftValue<Value> =
  | Value
  | null
  | (Value extends number ? string : never)
  | (Value extends string[] ? string : never);
export type ConfigSectionValues = Partial<{
  [Field in ConfigFieldName]: ConfigDraftValue<AdminConfigFlat[Field]>;
}>;
export type ConfigFieldValue = ConfigSectionValues[ConfigFieldName];

export interface FormCtx {
  control: Control<ConfigSectionValues>;
  get: <Group extends ConfigGroupKey, Field extends ConfigGroupField<Group>>(
    group: Group,
    field: Field,
  ) => ConfigFieldValue;
  isSaving: (field: ConfigFieldName) => boolean;
  /** Canonicalize one field into the local draft through its codec; no request is sent. */
  stage: <Group extends ConfigGroupKey, Field extends ConfigGroupField<Group>>(
    group: Group,
    field: Field,
    value: unknown,
  ) => ConfigFieldValue;
}

interface ConfigFieldCodec<WireValue> {
  readonly serverSchema: z.ZodType<WireValue>;
  readonly draftSchema: z.ZodType<unknown>;
  readonly canonicalize: (value: unknown) => WireValue | null;
}

type ConfigFieldCanonicalizer = Pick<ConfigFieldCodec<ConfigFieldValue>, 'canonicalize'>;

function fieldCodec<WireValue>(
  serverSchema: z.ZodType<WireValue>,
  canonicalize: (value: unknown) => WireValue | null,
): ConfigFieldCodec<WireValue> {
  return {
    serverSchema,
    canonicalize,
    draftSchema: z.unknown().superRefine((value, ctx) => {
      try {
        canonicalize(value);
      } catch (error) {
        ctx.addIssue({
          code: 'custom',
          message:
            error instanceof Error && error.message.startsWith('admin.config.')
              ? error.message
              : 'admin.config.invalid_value',
        });
      }
    }),
  };
}

function nullable(value: unknown): value is null {
  return value === null;
}

function canonicalString(value: unknown): string | null {
  if (nullable(value)) return null;
  if (typeof value !== 'string') throw new TypeError('admin.config.invalid_value');
  return value;
}

function canonicalNullableString(value: unknown): string | null {
  const parsed = canonicalString(value);
  return parsed === '' ? null : parsed;
}

function canonicalBoolean(value: unknown): boolean | null {
  if (nullable(value)) return null;
  if (typeof value !== 'boolean') throw new TypeError('admin.config.invalid_value');
  return value;
}

function canonicalInteger(value: unknown): number | null {
  if (nullable(value)) return null;
  if (typeof value === 'string') return parseBackendInteger(value);
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    throw new TypeError('admin.config.integer_invalid');
  }
  return value;
}

function canonicalNumber(value: unknown): number | null {
  if (nullable(value)) return null;
  if (typeof value === 'string') return parseBackendNumber(value);
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new TypeError('admin.config.number_invalid');
  }
  return value;
}

function canonicalStringArray(value: unknown): string[] | null {
  if (nullable(value)) return null;
  if (typeof value === 'string') return splitComma(value);
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    throw new TypeError('admin.config.invalid_value');
  }
  return value;
}

const safeIntegerSchema = z.number().refine(Number.isSafeInteger);
const finiteNumberSchema = z.number().refine(Number.isFinite);
const stringField = fieldCodec(z.string(), canonicalString);
const nullableStringField = fieldCodec(z.string().nullable(), canonicalNullableString);
const booleanField = fieldCodec(z.boolean(), canonicalBoolean);
const stringArrayField = fieldCodec(z.array(z.string()), canonicalStringArray);

function boundedIntegerField(minimum: number, maximum: number): ConfigFieldCodec<number> {
  const parse = (value: unknown): number | null => {
    const parsed = canonicalInteger(value);
    if (parsed === null) return null;
    if (parsed < minimum || parsed > maximum) {
      throw new TypeError('admin.config.invalid_value');
    }
    return parsed;
  };
  return fieldCodec(
    safeIntegerSchema.refine((value) => value >= minimum && value <= maximum),
    parse,
  );
}

function nullableBoundedIntegerField(
  minimum: number,
  maximum: number,
): ConfigFieldCodec<number | null> {
  const bounded = boundedIntegerField(minimum, maximum);
  return fieldCodec(bounded.serverSchema.nullable(), bounded.canonicalize);
}

function boundedNumberField(minimum: number): ConfigFieldCodec<number> {
  const parse = (value: unknown): number | null => {
    const parsed = canonicalNumber(value);
    if (parsed === null) return null;
    if (parsed < minimum) throw new TypeError('admin.config.invalid_value');
    return parsed;
  };
  return fieldCodec(
    finiteNumberSchema.refine((value) => value >= minimum),
    parse,
  );
}

function nullableBoundedNumberField(minimum: number): ConfigFieldCodec<number | null> {
  const bounded = boundedNumberField(minimum);
  return fieldCodec(bounded.serverSchema.nullable(), bounded.canonicalize);
}

const MAX_I32 = 2_147_483_647;
const MAX_DURATION_MINUTES = 365 * 24 * 60;
const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER;

function integerEnumField<const Values extends readonly number[]>(
  values: Values,
): ConfigFieldCodec<Values[number]> {
  const allowed = new Set<number>(values);
  const parse = (value: unknown): Values[number] | null => {
    const parsed = canonicalInteger(value);
    if (parsed === null) return null;
    if (!allowed.has(parsed)) throw new TypeError('admin.config.invalid_value');
    return parsed as Values[number];
  };
  return fieldCodec(
    safeIntegerSchema.refine((value) => allowed.has(value)) as z.ZodType<Values[number]>,
    parse,
  );
}

function stringEnumField<const Values extends readonly string[]>(
  values: Values,
): ConfigFieldCodec<Values[number]> {
  const allowed = new Set<string>(values);
  const parse = (value: unknown): Values[number] | null => {
    const parsed = canonicalString(value);
    if (parsed === null) return null;
    if (!allowed.has(parsed)) throw new TypeError('admin.config.invalid_value');
    return parsed as Values[number];
  };
  return fieldCodec(
    z.string().refine((value) => allowed.has(value)) as z.ZodType<Values[number]>,
    parse,
  );
}

type ConfigCodecGroups = {
  [Group in ConfigGroupKey]: {
    [Field in keyof AdminConfigGroups[Group]]: ConfigFieldCodec<AdminConfigGroups[Group][Field]>;
  };
};

/**
 * Single typed registry for config ownership, GET validation and draft→PATCH
 * conversion. A typo, a field in the wrong group, or a codec returning the
 * wrong wire type is a compile-time error here.
 */
const CONFIG_FIELD_CODECS = {
  site: {
    app_name: stringField,
    app_description: nullableStringField,
    app_url: nullableStringField,
    force_https: booleanField,
    logo: nullableStringField,
    subscribe_url: nullableStringField,
    subscribe_path: nullableStringField,
    tos_url: nullableStringField,
    stop_register: booleanField,
    try_out_plan_id: boundedIntegerField(0, MAX_I32),
    try_out_hour: boundedNumberField(0),
    currency: stringField,
    currency_symbol: stringField,
    legacy_hash_redirect_enable: booleanField,
  },
  safe: {
    email_verify: booleanField,
    email_gmail_limit_enable: booleanField,
    safe_mode_enable: booleanField,
    admin_mfa_force: booleanField,
    secure_path: stringField,
    email_whitelist_enable: booleanField,
    email_whitelist_suffix: stringArrayField,
    recaptcha_enable: booleanField,
    recaptcha_key: nullableStringField,
    recaptcha_site_key: nullableStringField,
    register_limit_by_ip_enable: booleanField,
    register_limit_count: boundedIntegerField(1, MAX_SAFE_INTEGER),
    register_limit_expire: boundedIntegerField(1, MAX_DURATION_MINUTES),
    password_limit_enable: booleanField,
    password_limit_count: boundedIntegerField(1, MAX_SAFE_INTEGER),
    password_limit_expire: boundedIntegerField(1, MAX_DURATION_MINUTES),
  },
  subscribe: {
    plan_change_enable: booleanField,
    reset_traffic_method: integerEnumField([0, 1, 2, 3, 4] as const),
    surplus_enable: booleanField,
    allow_new_period: booleanField,
    new_order_event_id: booleanField,
    renew_order_event_id: booleanField,
    change_order_event_id: booleanField,
    show_info_to_server_enable: booleanField,
    show_subscribe_method: integerEnumField([0, 1, 2] as const),
    show_subscribe_expire: boundedIntegerField(1, MAX_DURATION_MINUTES),
  },
  deposit: { deposit_bounus: stringArrayField },
  ticket: { ticket_status: integerEnumField([0, 1, 2] as const) },
  invite: {
    invite_force: booleanField,
    invite_commission: boundedIntegerField(0, MAX_I32),
    invite_gen_limit: boundedIntegerField(0, MAX_SAFE_INTEGER),
    invite_never_expire: booleanField,
    commission_first_time_enable: booleanField,
    commission_auto_check_enable: booleanField,
    commission_withdraw_limit: stringField,
    commission_withdraw_method: stringArrayField,
    withdraw_close_enable: booleanField,
    commission_distribution_enable: booleanField,
    commission_distribution_l1: nullableBoundedNumberField(0),
    commission_distribution_l2: nullableBoundedNumberField(0),
    commission_distribution_l3: nullableBoundedNumberField(0),
  },
  frontend: {
    frontend_theme_color: stringEnumField(['default', 'darkblue', 'black', 'green'] as const),
    frontend_background_url: nullableStringField,
  },
  server: {
    server_api_url: nullableStringField,
    server_token: nullableStringField,
    server_pull_interval: boundedIntegerField(1, MAX_I32),
    server_push_interval: boundedIntegerField(1, MAX_I32),
    server_node_report_min_traffic: boundedIntegerField(0, MAX_I32),
    server_device_online_min_traffic: boundedIntegerField(0, MAX_I32),
    device_limit_mode: booleanField,
  },
  email: {
    email_host: nullableStringField,
    email_port: nullableBoundedIntegerField(1, 65_535),
    email_encryption: nullableStringField,
    email_username: nullableStringField,
    email_password: nullableStringField,
    email_from_address: nullableStringField,
    email_template: stringField,
  },
  telegram: {
    telegram_bot_token: nullableStringField,
    telegram_bot_enable: booleanField,
    telegram_discuss_link: nullableStringField,
  },
  app: {
    windows_version: nullableStringField,
    windows_download_url: nullableStringField,
    macos_version: nullableStringField,
    macos_download_url: nullableStringField,
    android_version: nullableStringField,
    android_download_url: nullableStringField,
  },
} satisfies ConfigCodecGroups;

function objectKeys<ObjectType extends object>(
  value: ObjectType,
): Extract<keyof ObjectType, string>[] {
  return Object.keys(value) as Extract<keyof ObjectType, string>[];
}

type ConfigSectionFields = {
  [Group in ConfigGroupKey]: readonly ConfigGroupField<Group>[];
};

// Keep the group list explicit so adding a config group is a compile-time
// obligation. `objectKeys` preserves each concrete registry member's field
// union instead of erasing the group/field relationship through fromEntries.
export const SECTION_FIELDS = {
  site: objectKeys(CONFIG_FIELD_CODECS.site),
  safe: objectKeys(CONFIG_FIELD_CODECS.safe),
  subscribe: objectKeys(CONFIG_FIELD_CODECS.subscribe),
  deposit: objectKeys(CONFIG_FIELD_CODECS.deposit),
  ticket: objectKeys(CONFIG_FIELD_CODECS.ticket),
  invite: objectKeys(CONFIG_FIELD_CODECS.invite),
  frontend: objectKeys(CONFIG_FIELD_CODECS.frontend),
  server: objectKeys(CONFIG_FIELD_CODECS.server),
  email: objectKeys(CONFIG_FIELD_CODECS.email),
  telegram: objectKeys(CONFIG_FIELD_CODECS.telegram),
  app: objectKeys(CONFIG_FIELD_CODECS.app),
} satisfies ConfigSectionFields;

function createSectionSchema(
  group: ConfigGroupKey,
  source: 'server' | 'draft',
): z.ZodType<ConfigSectionValues, ConfigSectionValues> {
  const shape: Record<string, z.ZodType<unknown>> = {};
  for (const [field, codec] of Object.entries(CONFIG_FIELD_CODECS[group])) {
    shape[field] = (source === 'server' ? codec.serverSchema : codec.draftSchema).optional();
  }
  return z.strictObject(shape) as unknown as z.ZodType<ConfigSectionValues, ConfigSectionValues>;
}

export const SECTION_SCHEMAS = Object.fromEntries(
  (Object.keys(CONFIG_FIELD_CODECS) as ConfigGroupKey[]).map((group) => [
    group,
    createSectionSchema(group, 'draft'),
  ]),
) as Record<ConfigGroupKey, z.ZodType<ConfigSectionValues, ConfigSectionValues>>;

const SERVER_SECTION_SCHEMAS = Object.fromEntries(
  (Object.keys(CONFIG_FIELD_CODECS) as ConfigGroupKey[]).map((group) => [
    group,
    createSectionSchema(group, 'server'),
  ]),
) as Record<ConfigGroupKey, z.ZodType<ConfigSectionValues, ConfigSectionValues>>;

export function parseConfigServerSection(group: ConfigGroupKey, value: unknown) {
  return SERVER_SECTION_SCHEMAS[group].parse(value);
}

export function canonicalizeConfigDraftField<
  Group extends ConfigGroupKey,
  Field extends ConfigGroupField<Group>,
>(group: Group, field: Field, value: unknown): AdminConfigGroups[Group][Field] | null;
export function canonicalizeConfigDraftField(
  group: ConfigGroupKey,
  field: ConfigFieldName,
  value: unknown,
): ConfigFieldValue;
export function canonicalizeConfigDraftField(
  group: ConfigGroupKey,
  field: ConfigFieldName,
  value: unknown,
): ConfigFieldValue {
  const codecs: Partial<Record<ConfigFieldName, ConfigFieldCanonicalizer>> =
    CONFIG_FIELD_CODECS[group];
  const codec = codecs[field];
  if (!codec) throw new TypeError('admin.config.field_not_in_group');
  return codec.canonicalize(value);
}

// Titles resolve at render time so the active locale always wins.
export const SECTIONS: { key: ConfigGroupKey; title: (t: TFunction) => string }[] = [
  { key: 'site', title: (t) => t(($) => $.admin.config.sections.site) },
  { key: 'safe', title: (t) => t(($) => $.admin.config.sections.safe) },
  { key: 'subscribe', title: (t) => t(($) => $.admin.config.sections.subscribe) },
  { key: 'deposit', title: (t) => t(($) => $.admin.config.sections.deposit) },
  { key: 'ticket', title: (t) => t(($) => $.admin.config.sections.ticket) },
  { key: 'invite', title: (t) => t(($) => $.admin.config.sections.invite) },
  { key: 'frontend', title: (t) => t(($) => $.admin.config.sections.frontend) },
  { key: 'server', title: (t) => t(($) => $.admin.config.sections.server) },
  { key: 'email', title: (t) => t(($) => $.admin.config.sections.email) },
  { key: 'telegram', title: (t) => t(($) => $.admin.config.sections.telegram) },
  { key: 'app', title: (t) => t(($) => $.admin.config.sections.app) },
];
