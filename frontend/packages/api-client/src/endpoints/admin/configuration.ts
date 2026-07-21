import type {
  AdminConfig,
  AdminConfigPatch,
  AdminConfigPatchResult,
  InternalApiOperationMap,
} from '@v2board/types';
import type { ApiClient } from '../../client';
import { requestInternal } from '../../internal-operation';
import type { QueryRequestConfig } from './shared';

export type AdminTestMailResult = InternalApiOperationMap['adminTestMailSend']['response'];
type AdminConfigWire = InternalApiOperationMap['adminConfigGet']['response'];

function numericEnum<const Values extends readonly number[]>(
  value: number,
  values: Values,
  field: string,
): Values[number] {
  if (!values.includes(value)) throw new TypeError(`Unsupported ${field}: ${value}`);
  return value as Values[number];
}

const FRONTEND_THEME_COLORS = ['default', 'darkblue', 'black', 'green'] as const;

function frontendThemeColor(value: string): (typeof FRONTEND_THEME_COLORS)[number] {
  if (!(FRONTEND_THEME_COLORS as readonly string[]).includes(value)) {
    throw new TypeError(`Unsupported frontend theme color: ${value}`);
  }
  return value as (typeof FRONTEND_THEME_COLORS)[number];
}

function normalizeAdminConfig(config: AdminConfigWire): AdminConfig {
  const {
    ticket,
    deposit,
    invite,
    site,
    subscribe,
    frontend,
    server,
    email,
    telegram,
    app,
    safe,
    ...flat
  } = config;
  const normalizedTicket =
    ticket == null
      ? undefined
      : { ...ticket, ticket_status: numericEnum(ticket.ticket_status, [0, 1, 2], 'ticket status') };
  const normalizedSubscribe =
    subscribe == null
      ? undefined
      : {
          ...subscribe,
          reset_traffic_method: numericEnum(
            subscribe.reset_traffic_method,
            [0, 1, 2, 3, 4],
            'reset traffic method',
          ),
          show_subscribe_method: numericEnum(
            subscribe.show_subscribe_method,
            [0, 1, 2],
            'subscribe display method',
          ),
        };
  const normalizedFrontend =
    frontend == null
      ? undefined
      : { ...frontend, frontend_theme_color: frontendThemeColor(frontend.frontend_theme_color) };
  const normalizedEmail =
    email == null ? undefined : { ...email, email_template: email.email_template ?? 'default' };
  return {
    ...(normalizedTicket ?? {}),
    ...(deposit ?? {}),
    ...(invite ?? {}),
    ...(site ?? {}),
    ...(normalizedSubscribe ?? {}),
    ...(normalizedFrontend ?? {}),
    ...(server ?? {}),
    ...(normalizedEmail ?? {}),
    ...(telegram ?? {}),
    ...(app ?? {}),
    ...(safe ?? {}),
    ...flat,
    ...(normalizedTicket === undefined ? {} : { ticket: normalizedTicket }),
    ...(deposit == null ? {} : { deposit }),
    ...(invite == null ? {} : { invite }),
    ...(site == null ? {} : { site }),
    ...(normalizedSubscribe === undefined ? {} : { subscribe: normalizedSubscribe }),
    ...(normalizedFrontend === undefined ? {} : { frontend: normalizedFrontend }),
    ...(server == null ? {} : { server }),
    ...(normalizedEmail === undefined ? {} : { email: normalizedEmail }),
    ...(telegram == null ? {} : { telegram }),
    ...(app == null ? {} : { app }),
    ...(safe == null ? {} : { safe }),
  };
}

function explicitAdminConfigPath(securePath: string): string {
  const normalized = securePath.trim().replace(/^\/+|\/+$/g, '');
  if (!/^[A-Za-z0-9_-]{8,}$/.test(normalized)) {
    throw new TypeError('Invalid admin secure path');
  }
  return `/${encodeURIComponent(normalized)}/config`;
}

/** GET /{secure_path}/config `?group=` — dialect v2 bare grouped object (§6.1, W9). */
export const fetchConfig = async (client: ApiClient, group?: string, config?: QueryRequestConfig) =>
  normalizeAdminConfig(
    await requestInternal(client, 'adminConfigGet', {
      query: group ? { group } : {},
      ...config,
    }),
  );

/**
 * Probe a newly committed admin prefix without changing the page-lifetime
 * runtime config. This is used only while a secure-path PATCH is durably
 * pending: the old prefix can disappear before it can report the new active
 * revision, so activation must be confirmed through the new prefix itself.
 */
export const fetchConfigAtAdminPath = async (
  client: ApiClient,
  securePath: string,
  group?: string,
  config?: QueryRequestConfig,
) =>
  normalizeAdminConfig(
    await requestInternal(client, 'adminConfigGet', {
      contractUrlOverride: explicitAdminConfigPath(securePath),
      query: group ? { group } : {},
      ...config,
    }),
  );

/**
 * PATCH /{secure_path}/config — dialect v2 partial JSON body in §4.1 native
 * types (real booleans/arrays; the legacy `'[]'`-string empty-array hack is
 * dead) with §4.4 null-clear semantics for resettable fields; secure_path
 * always requires an explicit non-empty replacement. `expected_revision` is
 * the required positive token from the GET snapshot on which the patch is
 * based. 204 means the write fully activated;
 * 202 `{activation: "pending", revision}` means it is durable but not yet
 * active — the caller must observe that revision on GET, never resubmit (a resubmit would 409
 * `config_revision_conflict` on the now-stale revision).
 */
export const saveConfig = (
  client: ApiClient,
  data: AdminConfigPatch,
): Promise<AdminConfigPatchResult> =>
  requestInternal(client, 'adminConfigUpdate', {
    data,
  }).then((body) => body ?? ({ activation: 'applied' } as const));

/** GET /{secure_path}/email-templates — dialect v2 bare array (§6.1, W9). */
export const getEmailTemplate = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminEmailTemplatesList', {
    ...config,
  });

/** POST /{secure_path}/telegram-webhook — dialect v2, 204 (§6.1, W9). */
export const setTelegramWebhook = (client: ApiClient, telegram_bot_token?: string) =>
  requestInternal(client, 'adminTelegramWebhookSet', {
    data: telegram_bot_token === undefined ? {} : { telegram_bot_token },
  });

/**
 * POST /{secure_path}/test-mail — dialect v2 bare `{sent, log}` (§6.1, W9):
 * the legacy `{data: true, log}` envelope became a named object; failures are
 * problems (400 mail_sender_not_configured/mail_invalid, 502 mail_send_failed).
 */
export const testSendMail = (client: ApiClient) => requestInternal(client, 'adminTestMailSend', {});

/** GET /{secure_path}/account/mfa — the caller's own two-factor state (§6.10). */
export const fetchAccountMfa = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminAccountMfaGet', {
    ...config,
  });

/**
 * POST /{secure_path}/account/mfa/totp — start a pending TOTP enrollment
 * (§6.10). The provisioning secret in the response is shown exactly once.
 */
export const setupAccountTotp = (client: ApiClient) =>
  requestInternal(client, 'adminAccountMfaTotpSetup', {});

/** POST /{secure_path}/account/mfa/totp/confirm — enable with a live code; 204 (§6.10). */
export const confirmAccountTotp = (client: ApiClient, code: string) =>
  requestInternal(client, 'adminAccountMfaTotpConfirm', {
    data: { code },
  });

/** POST /{secure_path}/account/mfa/totp/disable — remove with a live code; 204 (§6.10). */
export const disableAccountTotp = (client: ApiClient, code: string) =>
  requestInternal(client, 'adminAccountMfaTotpDisable', {
    data: { code },
  });
