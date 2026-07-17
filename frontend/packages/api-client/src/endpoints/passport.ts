// The guest auth family — dialect v2 (docs/api-dialect.md §5.2, Appendix A
// §W2): JSON bodies against the `/auth/*` routes, bare success bodies, and
// problem+json failures. The module keeps its historical `passport` name so
// call sites stay stable while the wire moved off `/passport/*`.
import type { ApiClient } from '../client';
import { authDataSchema, noContentSchema, stepUpGrantSchema } from '../contracts';

export interface LoginPayload {
  email: string;
  password: string;
}

export interface RegisterPayload extends LoginPayload {
  invite_code?: string;
  email_code?: string;
  recaptcha_data?: string;
}

export interface ForgetPayload {
  email: string;
  email_code: string;
  password: string;
}

export interface SendEmailVerifyPayload {
  email: string;
  recaptcha_data?: string;
  /** `false` for register (email-exists check), `true` for password reset. */
  is_forget?: boolean;
}

export interface TokenLoginPayload {
  verify: string;
}

export const login = (client: ApiClient, payload: LoginPayload) =>
  client.request({
    url: '/auth/login',
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: authDataSchema,
  });

/** 201 Created with the same bare auth body as login (§5.2). */
export const register = (client: ApiClient, payload: RegisterPayload) =>
  client.request({
    url: '/auth/register',
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: authDataSchema,
  });

/** 204 on success; failures surface as coded problems (§3.4). */
export const forget = (client: ApiClient, payload: ForgetPayload) =>
  client.request({
    url: '/auth/password-reset',
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: noContentSchema,
  });

/** 204 on success; rate limits and policy failures are coded problems. */
export const sendEmailVerify = (client: ApiClient, payload: SendEmailVerifyPayload) =>
  client.request({
    url: '/auth/email-codes',
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: noContentSchema,
  });

/**
 * The SPA one-time `?verify=` exchange (POST, §5.2): a dead or malformed
 * token rejects as a 400 `invalid_token` problem instead of a null body.
 */
export const tokenLogin = (client: ApiClient, payload: TokenLoginPayload) =>
  client.request({
    url: '/auth/token-login',
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: authDataSchema,
  });

export interface StepUpPayload {
  password: string;
}

/**
 * Re-verify the signed-in admin/staff password for privileged writes. The
 * returned token rides on subsequent requests as the `x-v2board-step-up`
 * header (client option getStepUpToken) until `expires_in` elapses.
 */
export const stepUp = (client: ApiClient, payload: StepUpPayload) =>
  client.request({
    url: '/auth/step-up',
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: stepUpGrantSchema,
  });
