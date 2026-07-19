import type { AdminConfig, AdminConfigFlat, AdminConfigPatchResult } from '@v2board/types';
import { z, type output } from 'zod';
import type { ApiClient } from '../../client';
import {
  adminConfigSchema,
  configActivationPendingSchema,
  mfaStatusSchema,
  noContentSchema,
  stringArraySchema,
  testMailResultSchema,
  totpProvisioningSchema,
} from '../../contracts';
import type { QueryRequestConfig } from './shared';

export type AdminTestMailResult = output<typeof testMailResultSchema>;

function normalizeAdminConfig(config: output<typeof adminConfigSchema>): AdminConfig {
  return {
    ...(config.ticket ?? {}),
    ...(config.deposit ?? {}),
    ...(config.invite ?? {}),
    ...(config.site ?? {}),
    ...(config.subscribe ?? {}),
    ...(config.frontend ?? {}),
    ...(config.server ?? {}),
    ...(config.email ?? {}),
    ...(config.telegram ?? {}),
    ...(config.app ?? {}),
    ...(config.safe ?? {}),
    ...config,
  };
}

/** GET /{secure_path}/config `?group=` — dialect v2 bare grouped object (§6.1, W9). */
export const fetchConfig = async (client: ApiClient, group?: string, config?: QueryRequestConfig) =>
  normalizeAdminConfig(
    await client.request({
      url: client.resolveAdminPath('/config'),
      method: 'GET',
      dialect: 'v2',
      params: group ? { group } : undefined,
      responseSchema: adminConfigSchema,
      ...config,
    }),
  );

/**
 * PATCH /{secure_path}/config — dialect v2 partial JSON body in §4.1 native
 * types (real booleans/arrays; the legacy `'[]'`-string empty-array hack is
 * dead) with §4.4 null-clear semantics. 204 means the write fully activated;
 * 202 `{activation: "pending"}` means it is durable but not yet active — the
 * caller must refetch, never resubmit (a resubmit would 409
 * `config_revision_conflict` on the now-stale revision).
 */
export const saveConfig = (
  client: ApiClient,
  data: Partial<AdminConfigFlat>,
): Promise<AdminConfigPatchResult> =>
  client
    .request({
      url: client.resolveAdminPath('/config'),
      method: 'PATCH',
      dialect: 'v2',
      data,
      responseSchema: z.union([noContentSchema, configActivationPendingSchema]),
    })
    .then((body) => ({ activation: body?.activation ?? 'applied' }));

/** GET /{secure_path}/email-templates — dialect v2 bare array (§6.1, W9). */
export const getEmailTemplate = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/email-templates'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: stringArraySchema,
    ...config,
  });

/** POST /{secure_path}/telegram-webhook — dialect v2, 204 (§6.1, W9). */
export const setTelegramWebhook = (client: ApiClient, telegram_bot_token?: string) =>
  client.request({
    url: client.resolveAdminPath('/telegram-webhook'),
    method: 'POST',
    dialect: 'v2',
    data: telegram_bot_token === undefined ? {} : { telegram_bot_token },
    responseSchema: noContentSchema,
  });

/**
 * POST /{secure_path}/test-mail — dialect v2 bare `{sent, log}` (§6.1, W9):
 * the legacy `{data: true, log}` envelope became a named object; failures are
 * problems (400 mail_sender_not_configured/mail_invalid, 502 mail_send_failed).
 */
export const testSendMail = (client: ApiClient) =>
  client.request({
    url: client.resolveAdminPath('/test-mail'),
    method: 'POST',
    dialect: 'v2',
    responseSchema: testMailResultSchema,
  });

/** GET /{secure_path}/account/mfa — the caller's own two-factor state (§6.10). */
export const fetchAccountMfa = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/account/mfa'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: mfaStatusSchema,
    ...config,
  });

/**
 * POST /{secure_path}/account/mfa/totp — start a pending TOTP enrollment
 * (§6.10). The provisioning secret in the response is shown exactly once.
 */
export const setupAccountTotp = (client: ApiClient) =>
  client.request({
    url: client.resolveAdminPath('/account/mfa/totp'),
    method: 'POST',
    dialect: 'v2',
    responseSchema: totpProvisioningSchema,
  });

/** POST /{secure_path}/account/mfa/totp/confirm — enable with a live code; 204 (§6.10). */
export const confirmAccountTotp = (client: ApiClient, code: string) =>
  client.request({
    url: client.resolveAdminPath('/account/mfa/totp/confirm'),
    method: 'POST',
    dialect: 'v2',
    data: { code },
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/account/mfa/totp/disable — remove with a live code; 204 (§6.10). */
export const disableAccountTotp = (client: ApiClient, code: string) =>
  client.request({
    url: client.resolveAdminPath('/account/mfa/totp/disable'),
    method: 'POST',
    dialect: 'v2',
    data: { code },
    responseSchema: noContentSchema,
  });
