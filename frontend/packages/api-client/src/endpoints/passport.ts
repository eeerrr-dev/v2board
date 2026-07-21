// The guest auth family — dialect v2 (docs/api-dialect.md §5.2, Appendix A
// §W2): JSON bodies against the `/auth/*` routes, bare success bodies, and
// problem+json failures. The module keeps its historical `passport` name so
// call sites stay stable while the wire moved off `/passport/*`.
import type { InternalApiOperationMap } from '@v2board/types';
import type { ApiClient } from '../client';
import { requestInternal } from '../internal-operation';

export type LoginPayload = InternalApiOperationMap['authLogin']['request'];
export type RegisterPayload = InternalApiOperationMap['authRegister']['request'];
export type ForgetPayload = InternalApiOperationMap['authPasswordReset']['request'];
export type SendEmailVerifyPayload = InternalApiOperationMap['authEmailCodes']['request'];
export type TokenLoginPayload = InternalApiOperationMap['authTokenLogin']['request'];

export const login = (client: ApiClient, payload: LoginPayload) =>
  requestInternal(client, 'authLogin', {
    data: payload,
  });

/** 201 Created with the same bare auth body as login (§5.2). */
export const register = (client: ApiClient, payload: RegisterPayload) =>
  requestInternal(client, 'authRegister', {
    data: payload,
  });

/** 204 on success; failures surface as coded problems (§3.4). */
export const forget = (client: ApiClient, payload: ForgetPayload) =>
  requestInternal(client, 'authPasswordReset', {
    data: payload,
  });

/** 204 on success; rate limits and policy failures are coded problems. */
export const sendEmailVerify = (client: ApiClient, payload: SendEmailVerifyPayload) =>
  requestInternal(client, 'authEmailCodes', {
    data: payload,
  });

/**
 * The SPA one-time `?verify=` exchange (POST, §5.2): a dead or malformed
 * token rejects as a 400 `invalid_token` problem instead of a null body.
 */
export const tokenLogin = (client: ApiClient, payload: TokenLoginPayload) =>
  requestInternal(client, 'authTokenLogin', {
    data: payload,
  });

export type StepUpPayload = InternalApiOperationMap['authStepUp']['request'];

/**
 * Re-verify the signed-in admin/staff password for privileged writes. The
 * returned token rides on subsequent requests as the `x-v2board-step-up`
 * header (client option getStepUpToken) until `expires_in` elapses.
 */
export const stepUp = (client: ApiClient, payload: StepUpPayload) =>
  requestInternal(client, 'authStepUp', {
    data: payload,
  });
