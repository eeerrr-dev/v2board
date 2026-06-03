import type { AuthData } from '@v2board/types';
import type { ApiClient } from '../client';

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
  isforget?: 0 | 1;
}

export interface TokenLoginPayload {
  verify: string;
  redirect?: string;
}

export const login = (client: ApiClient, payload: LoginPayload) =>
  client.request<AuthData>({ url: '/passport/auth/login', method: 'POST', data: payload });

export const register = (client: ApiClient, payload: RegisterPayload) =>
  client.request<AuthData>({ url: '/passport/auth/register', method: 'POST', data: payload });

export const forget = (client: ApiClient, payload: ForgetPayload) =>
  client.request<true>({ url: '/passport/auth/forget', method: 'POST', data: payload });

export const sendEmailVerify = (client: ApiClient, payload: SendEmailVerifyPayload) =>
  client.request<true>({ url: '/passport/comm/sendEmailVerify', method: 'POST', data: payload });

export const token2Login = (client: ApiClient, payload: TokenLoginPayload) =>
  client.request<AuthData | null>({ url: '/passport/auth/token2Login', method: 'GET', params: payload });
