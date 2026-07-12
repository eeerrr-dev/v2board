import type { ApiClient, ApiRequestConfig } from '../client';
import { guestConfigSchema } from '../contracts';

export const config = (client: ApiClient, request?: Pick<ApiRequestConfig, 'signal'>) =>
  client.request({
    url: '/guest/comm/config',
    method: 'GET',
    responseSchema: guestConfigSchema,
    ...request,
  });
